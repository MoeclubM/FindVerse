# FindVerse

[English](README.md) | [简体中文](README.zh-CN.md)

FindVerse 是一套可自部署的搜索系统，主站现在由 bootstrap、blob-storage、控制面、任务 API、调度器、projector、查询 API、Web 界面和独立爬虫节点组成。它的目标不是一开始就做成很重的平台，而是在单机部署足够简单的前提下，保留后续扩展抓取、索引和搜索链路的空间。

## 功能概览

- 提供公开搜索、搜索建议和开发者搜索 API
- 提供管理控制台，用于查看爬虫节点、抓取规则、任务和文档
- 爬虫节点可独立扩缩容、独立升级
- 支持递归抓取范围和单页发现链接数量限制
- 支持可选的 OpenAI 兼容 LLM 过滤
- 主站默认使用 Docker Compose 部署

## 仓库结构

- `apps/web`：`/`、`/dev`、`/console` 对应的 React 前端
- `services/control-api`：管理员、开发者、规则、任务、文档管理
- `services/query-api`：公开搜索、建议搜索、开发者搜索
- `services/task-api`：crawler claim/report/heartbeat 入口和任务写侧
- `services/scheduler`：规则展开、超时回收、重试与 recrawl 调度
- `services/projector`：staged ingest 恢复与投影执行器
- `services/blob-storage`：本地对象存储 HTTP 服务
- `services/bootstrap`：一次性 migration 与索引初始化入口
- `services/crawler`：爬虫 worker 与本地抓取工具
- `services/api`：供两个 API 复用的后端公共库

## 快速启动

1. 复制环境变量模板。

```bash
cp .env.example .env
```

2. 至少修改 `.env` 里的这些值。

- `FINDVERSE_FRONTEND_ORIGIN`
- `FINDVERSE_LOCAL_ADMIN_PASSWORD`
- `FINDVERSE_POSTGRES_PASSWORD`

3. 构建并启动主站。

```bash
docker compose up -d --build
```

主站数据会持久化到 `./data`，Docker 在启动时会自动创建这些目录。`bootstrap` 会在主站启动时自动执行 migration、初始化 OpenSearch alias、写入默认系统配置，并在当前 alias 还是空的情况下自动回填旧 blob、把 PostgreSQL 里的文档重建进新的 OpenSearch alias。

旧版 `dev_auth_store.json` 和 `developer_store.json` 不再在启动时自动导入。如果你还保留这两份旧数据，先执行 `findverse-control-api migrate-legacy --dev-auth-store <路径> --developer-store <路径>`，再启动新主站。如果同时传了 `--blob-storage-url` 或设置了 `FINDVERSE_BLOB_STORAGE_URL`，这个命令也会顺手把旧版文档正文和抓取结果补齐到新的 blob 存储布局；不传也没关系，后续由 `bootstrap` 自动补齐。

## 爬虫节点

先在 `/console -> Settings` 设置一组共享 crawler 认证密钥，然后直接从 GitHub 安装或更新 crawler：

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server https://search.example.com/api --crawler-key "<crawler-key>" --max-jobs 16 --skip-browser-install
```

安装脚本同时支持 `x86_64/amd64` 和 `aarch64/arm64` Linux 主机。默认总是下载最新 GitHub Release，也可以通过 `--version <tag>` 固定版本。首次安装时会在本地自动生成 `crawler_id`，并写入 `/etc/findverse-crawler/crawler.env`。以后重复执行这条命令就是原地更新，并继续复用这份本地 id。

如果你要在灰度时固定某个正式版本：

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server https://search.example.com/api --crawler-key "<crawler-key>" --version v0.0.15 --max-jobs 16 --skip-browser-install
```

## 开发说明

- 主站部署命令就是 `docker compose up -d --build`
- crawler 流量走 `web -> task-api`，控制台和开发者流量走 `web -> control-api`
- 生产环境建议 crawler 作为宿主机服务独立运行，不要并入主站 compose
- 当前 CI 只跑 `.github/workflows/_validate.yml` 里的常规校验
- Release 会同时发布 `x86_64` 和 `arm64` 包，`install-crawler.sh` 直接消费这些正式产物

## 文档

- 部署与运维说明：[docs/deployment.md](docs/deployment.md)
- 架构说明：[docs/architecture.md](docs/architecture.md)
