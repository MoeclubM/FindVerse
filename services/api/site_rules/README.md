# Site Rules

`services/api/site_rules` 使用可编辑的 `TOML` 规则文件，分为三层。
管理端负责读取这些默认规则，并通过心跳接口向爬虫下发规则包：

1. `platforms/*.toml`
用于识别通用平台或程序，并映射到平台预设。

2. `platform-presets/*.toml`
用于定义平台级抓取预设，例如 JS 渲染偏好、结构化发现源和页面动作规则。

3. `sites/*.toml`
用于定义特定站点预设，可直接匹配特定站点，也可通过 `extends` 继承平台预设。

若平台识别与站点预设都未命中，则回退到内置 `unknown` 预设，等价于全量抓取。

可用字段如下：

- `id`
- `priority`
- `preset`
- `extends`
- `[match]`
- `[render] prefer_js = true|false`
- `[discover] sources = ["/sitemap.xml", "/feed"]`
- `default_action = "allow_index_discover" | "allow_index_only" | "deny"`
- `[[rules]]`

页面规则可使用的条件如下：

- `hosts`
- `host_suffixes`
- `path_exacts`
- `path_prefixes`
- `path_regex`
- `query_keys`
- `query_values`
- `query_value_prefixes`

页面动作语义如下：

- `allow_index_discover`
允许索引正文，也允许继续发现链接。

- `allow_index_only`
允许索引正文，但不继续发现链接。

- `deny`
不索引正文，也不继续发现链接。
