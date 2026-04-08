# FindVerse

[English](README.md) | [简体中文](README.zh-CN.md)

FindVerse 是一套可自部署的搜索系统，采用 Docker 化控制层和独立爬虫节点。控制层可以在单机快速拉起，爬虫节点则可以独立扩缩容和独立升级。

## 快速启动

1. 复制环境变量模板。

```bash
cp .env.example .env
```

2. 至少修改 `.env` 里的这些值。

- `FINDVERSE_FRONTEND_ORIGIN`
- `FINDVERSE_LOCAL_ADMIN_PASSWORD`
- `FINDVERSE_POSTGRES_PASSWORD`

3. 启动控制层。

```bash
docker compose up -d --build
```

4. 先在 `/console -> Settings` 设置一组共享 crawler 认证密钥，再安装 crawler 节点。

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server https://search.example.com/api --crawler-key "<crawler-key>" --max-jobs 16 --skip-browser-install
```

控制层数据保存在 `./data`。Docker Compose 只负责控制层，crawler 节点需要作为独立宿主机服务部署。如果某个 crawler 节点还停留在拆包之前的旧版本，需要先手动执行一次 `install-crawler.sh`，之后控制台远程升级才能继续正常工作。若机器内存偏紧，优先用 `COMPOSE_PARALLEL_LIMIT=1 docker compose up -d --build`。现在 Rust 服务镜像默认也会使用 `FINDVERSE_CARGO_BUILD_JOBS=1`。

## 文档

- 部署与运维说明：[docs/deployment.md](docs/deployment.md)
- 架构说明：[docs/architecture.md](docs/architecture.md)
