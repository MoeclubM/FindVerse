use std::{process, time::Duration};

use chrono::Utc;
use redis::{
    Client as RedisClient,
    streams::{
        StreamAutoClaimOptions, StreamAutoClaimReply, StreamId, StreamReadOptions, StreamReadReply,
    },
};
use uuid::Uuid;

use crate::error::ApiError;

const DEFAULT_TASK_BUS_STREAM_KEY: &str = "findverse:task-bus";
const DEFAULT_TASK_BUS_GROUP: &str = "findverse-projector";
const DEFAULT_TASK_BUS_MAXLEN: usize = 4_096;

#[derive(Debug, Clone)]
pub struct TaskBus {
    redis_client: RedisClient,
    stream_key: String,
    consumer_group: String,
    consumer_name: String,
}

impl TaskBus {
    pub fn new(redis_client: RedisClient) -> Self {
        Self {
            redis_client,
            stream_key: DEFAULT_TASK_BUS_STREAM_KEY.to_string(),
            consumer_group: DEFAULT_TASK_BUS_GROUP.to_string(),
            consumer_name: format!("projector-{}-{}", process::id(), Uuid::now_v7()),
        }
    }

    pub async fn publish(
        &self,
        kind: impl Into<String>,
        payload: serde_json::Value,
    ) -> Result<(), ApiError> {
        let kind = kind.into();
        let created_at = Utc::now();
        let encoded_payload =
            serde_json::to_string(&payload).map_err(|error| ApiError::Internal(error.into()))?;
        let mut conn = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        redis::cmd("XADD")
            .arg(&self.stream_key)
            .arg("MAXLEN")
            .arg("~")
            .arg(DEFAULT_TASK_BUS_MAXLEN)
            .arg("*")
            .arg("kind")
            .arg(kind)
            .arg("payload")
            .arg(encoded_payload)
            .arg("created_at")
            .arg(created_at.to_rfc3339())
            .query_async::<String>(&mut conn)
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        Ok(())
    }

    pub async fn read_batch(
        &self,
        batch_size: usize,
        timeout: Duration,
        min_idle: Duration,
    ) -> Result<Vec<String>, ApiError> {
        let mut conn = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;
        self.ensure_group(&mut conn).await?;

        let mut messages = self
            .reclaim_stale_messages(&mut conn, batch_size, min_idle)
            .await?;
        if messages.len() >= batch_size {
            return Ok(messages);
        }

        let remaining = batch_size - messages.len();
        let read_options = StreamReadOptions::default()
            .count(remaining)
            .block(duration_to_millis(timeout))
            .group(&self.consumer_group, &self.consumer_name);
        let reply: Option<StreamReadReply> = redis::cmd("XREADGROUP")
            .arg(&read_options)
            .arg("STREAMS")
            .arg(&self.stream_key)
            .arg(">")
            .query_async(&mut conn)
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        messages.extend(decode_stream_read_reply(reply)?);
        Ok(messages)
    }

    pub async fn ack(&self, message_ids: &[String]) -> Result<(), ApiError> {
        if message_ids.is_empty() {
            return Ok(());
        }

        let mut conn = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;
        self.ensure_group(&mut conn).await?;

        redis::cmd("XACK")
            .arg(&self.stream_key)
            .arg(&self.consumer_group)
            .arg(message_ids)
            .query_async::<usize>(&mut conn)
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        Ok(())
    }

    async fn ensure_group(
        &self,
        conn: &mut redis::aio::MultiplexedConnection,
    ) -> Result<(), ApiError> {
        let result: redis::RedisResult<()> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(&self.stream_key)
            .arg(&self.consumer_group)
            .arg("0-0")
            .arg("MKSTREAM")
            .query_async(conn)
            .await;

        match result {
            Ok(()) => Ok(()),
            Err(error) if error.code() == Some("BUSYGROUP") => Ok(()),
            Err(error) => Err(ApiError::Internal(error.into())),
        }
    }

    async fn reclaim_stale_messages(
        &self,
        conn: &mut redis::aio::MultiplexedConnection,
        batch_size: usize,
        min_idle: Duration,
    ) -> Result<Vec<String>, ApiError> {
        let mut messages = Vec::new();
        let mut start_id = "0-0".to_string();

        while messages.len() < batch_size {
            let reply: StreamAutoClaimReply = redis::cmd("XAUTOCLAIM")
                .arg(&self.stream_key)
                .arg(&self.consumer_group)
                .arg(&self.consumer_name)
                .arg(duration_to_millis(min_idle))
                .arg(&start_id)
                .arg(StreamAutoClaimOptions::default().count(batch_size - messages.len()))
                .query_async(conn)
                .await
                .map_err(|error| ApiError::Internal(error.into()))?;

            let claimed = reply.claimed;
            let next_stream_id = reply.next_stream_id;
            messages.extend(decode_stream_ids(claimed)?);

            if next_stream_id == start_id || next_stream_id == "0-0" {
                break;
            }
            start_id = next_stream_id;
        }

        Ok(messages)
    }
}

fn decode_stream_read_reply(reply: Option<StreamReadReply>) -> Result<Vec<String>, ApiError> {
    let mut messages = Vec::new();
    for key in reply.unwrap_or_default().keys {
        messages.extend(decode_stream_ids(key.ids)?);
    }
    Ok(messages)
}

fn decode_stream_ids(ids: Vec<StreamId>) -> Result<Vec<String>, ApiError> {
    ids.into_iter().map(decode_stream_id).collect()
}

fn decode_stream_id(entry: StreamId) -> Result<String, ApiError> {
    if entry.id.is_empty() {
        return Err(ApiError::Internal(anyhow::anyhow!(
            "task bus message missing stream id"
        )));
    }

    Ok(entry.id)
}

fn duration_to_millis(duration: Duration) -> usize {
    duration.as_millis().clamp(1, usize::MAX as u128) as usize
}
