# FindVerse

[English](README.md) | [简体中文](README.zh-CN.md)

FindVerse 是一套可自部署的搜索系统，主站由控制面、查询 API、Web 界面和独立爬虫节点组成。它的目标不是一开始就做成很重的平台，而是在单机部署足够简单的前提下，保留后续扩展抓取、索引和搜索链路的空间。

## 功能概览

- 提供公开搜索、搜索建议和开发者搜索 API
- 提供管理控制台，用于查看爬虫节点、抓取规则、任务和文档
- 爬虫节点可独立扩缩容、独立升级
- 支持递归抓取范围和单页发现链接数量限制
- 支持可选的 OpenAI 兼容 LLM 过滤
- 主站默认使用 Docker Compose 部署

## 仓库结构

- `apps/web`：`/`、`/dev`、`/console` 对应的 React 前端
- `services/control-api`：管理员、开发者、爬虫控制、frontier、任务、规则、文档管理
- `services/query-api`：公开搜索、建议搜索、开发者搜索
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

主站数据会持久化到 `./data`，Docker 在启动时会自动创建这些目录。

## 爬虫节点

先在 `/console -> Settings` 设置一组共享 crawler 认证密钥，然后直接从 GitHub 安装或更新 crawler：

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server https://search.example.com/api --crawler-key "<crawler-key>" --channel release --concurrency 16 --skip-browser-install
```

首次安装时会在本地自动生成 `crawler_id`，并写入 `/etc/findverse-crawler/crawler.env`。以后重复执行这条命令就是原地更新，并继续复用这份本地 id。

只有你明确要跟最新成功的 CI 开发构建时，才用 `dev`：

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo env GITHUB_TOKEN=<TOKEN> bash -s -- --server https://search.example.com/api --crawler-key "<crawler-key>" --channel dev --concurrency 16 --skip-browser-install
```

- `--channel release` 不需要 token
- `--channel dev` 需要 `GITHUB_TOKEN`，因为 GitHub Actions artifact 下载必须走已认证 API

## 开发说明

- 主站部署命令就是 `docker compose up -d --build`
- 生产环境建议 crawler 作为宿主机服务独立运行，不要并入主站 compose
- 当前 CI 只跑 `.github/workflows/_validate.yml` 里的常规校验
- `install-crawler.sh --channel dev` 使用独立 workflow 产出的 crawler 开发构建

## 文档

- 部署与运维说明：[docs/deployment.md](docs/deployment.md)
- 架构说明：[docs/architecture.md](docs/architecture.md)
