[English](README.md) | [한국어](README.ko.md) | [日本語](README.ja.md) | **简体中文**

# receiver

聊天接收器 `bmo-receiver` 负责处理任何聊天服务交来的消息。它保存房间及其消息、每个房间的规则、权限和窗口，
以及房间中智能体要回应的发言和静默指令的队列。它构建承载一个回合的信封，把回合交给代理服务（agency）的会话，
把智能体的回答发回房间，并唤醒房间发言、静默指令或任务结束所需要的智能体。所有聊天服务都通过基础库
`bmo-chat-client` 以同样的方式接入；其他所有单元则按 `docs/api.md` 和 `docs/openapi.json` 的规定通过 HTTP 访问。

## 目录结构

| 目录 | 内容 |
|---|---|
| `src/` | 服务自身的包：接收器对消息的判断以及接手它的唤醒（`receiver/`；唤醒本身的规则、要唤醒的智能体、所有者对任务提问的回答、休息闸门以及听取谁的发言，位于 `receiver/wake/` 下），被唤醒的回合所经历的轮次及其组成部分（`chat/`；对话、窗口、指令环节、房间块、回合的提示词、回答的形式、一个人所拥有房间的线索、回合记下的记录和语音通知的声音、安静房间的触碰、折叠、接收器所响应的大脑整合、被替换的会话留下的交接、房间的健康状态、重启，以及生成回答的指令流水线及其各阶段），房间获得回应的通道（`roads/`），删除房间时读取的房间记录（`room_record/`），事件流、实时看板以及订阅它们的每个页面（`events/`；为大量拉取消息的调用方准备的中继专线位于 `events/relay`），存储（迁移位于 `src/store/migration/` 下）及其在数据根目录下的文件夹，控制台、send 工具和 `bmo` 命令访问它的路由（`routes/`），`bmo` 分派给它的名词（`command`），`bmo` 命令针对其自身记录执行的动词（`verbs/`，包括向文档已移动的聊天发出重新加载通知），收听代理服务事件并把它们交给其效果所属部分的唯一事件流（`hearing`），告知所有者会话的工作者相关事实（`workers`），用来切分文本的读取（`dashes`、`web_address`、`text`、`form`、`json`、`run_lines`、`rounds`），从消息文字中识别的密钥形态以及清除了它们的保留行（`secrets`），以及服务启动时读取的配置（`config`）、状态和启动时的工作（`service_start`）、工具找到它的位置（`tool_env`）和 `src/bin/` 下的服务二进制文件 |
| `.cargo/` | 本工作区的 Cargo 设置：测试套件运行时遵守的测试线程上限，使每个测试的耗时保持在闸门监视的限制之内 |
| `deploy/` | 服务的用户单元，部署时从部署树链接它，并作为仓库唯一的服务重启 |
| `crates/` | `receiver-client`：以 Rust 类型定义固定 API 的请求和响应体，每条路由对应一个调用，并包含由此生成的 OpenAPI 文档，以及从同一文档生成的 TypeScript 包（`crates/receiver-client/ts/`）；`receiver-codes`：把接收器给出的所有拒绝、事实和固定文案定义为带结构化字段的代码；`receiver-policy`：`policy/` 的加载器；`receiver-ports`：接收器调用的每个单元的 trait 和 HTTP 客户端；`room-calls`：接收器的 send 工具以及执行相同发送的 `bmo` 命令向运行中的接收器请求的内容，包括操作及其参数、请求者、调用读取的回合、在房间中读取的流程、限制在上限内的读取，以及从数据根目录找到接收器的位置；`tool-call`：工具宿主调用协议的接收器一侧实现，由绑定到本服务的工具共用 |
| `tools/` | 绑定到本服务的工具，各自带有 `tool.toml`：`send` 是智能体在房间中发言的入口；`rooms` 是仅存在于记录中的智能体房间，智能体可以打开、读取它们并指定其应答者；`wait` 是由 `bmo use` 独立运行的提醒和监视 |
| `policy/` | 接收器的固定数值和文案，其中也包括用于匹配命令的韩语词语 |
| `tests/` | 包唯一的测试入口 `main.rs`：跨整个仓库的检查、每个文件一个模块的服务自身测试，以及模块 `tests/integration/` 中的集成测试；以其他单元的语言提问的测试文案作为数据放在 `tests/fixtures/` 下 |
| `docs/` | 关于服务行为的规范文档 |

根目录下有同时也是服务包的 Cargo 工作区清单、`VERSION` 中的版本、`testbed.toml`（测试平台关于如何测试、检查、
构建和部署本仓库的声明）、`install.toml`（在新机器上安装所需内容的声明，由 `bmo install` 读取）、`rustfmt.toml`、`CLAUDE.md`、`settings.example.toml`（签入的配置示例，部署自身的配置
文件位于数据根目录下）以及 `korean-ledger.json`（记录代码文件中保留的韩语，测试平台的韩语检查以它为准检查每个
文件；本仓库的代码文件中没有任何韩语）。

## 文档

| 文档 | 内容 |
|---|---|
| `docs/boundary-api.md` | 边界：接收器提供的每条路由、它对其他单元发起的每个调用，以及它返回的每个代码 |
| `docs/receiver.md` | 接收器承担的内容、唯一的硬性规则、所有者的话、事件流、删除房间注册、身份以及联系所有者的方式 |
| `docs/wake.md` | 为谁唤醒智能体的会话、房间保持的窗口、等待、回复以及房间的队列 |
| `docs/bmo-window.md` | 每个回合都经过的同一个窗口：队列、所有进入通道、静默指令、随行的行、回合携带的内容、开场、触碰、输入的一行、文档重新加载、事件 |
| `docs/unified-bmo.md` | 一个主管在多个房间中使用同一个会话：房间切换、私聊中的控制台对话、替换会话、默认访问权限和委派 |
| `docs/instruction.md` | 指令流水线：回答即消息的消息回合、消息发出前经过的阶段链及其文本、最终检查，以及轮次日志 |
| `docs/answer-checks.md` | 回答发出前要通过的检查 |
| `docs/voiced-notices.md` | 系统以自身名义告知房间的方式，以及保持固定的少数几行 |
| `docs/rooms.md` | 每个房间的开关、基础设施房间、委派和回复预算 |
| `docs/messages.md` | 进出的消息：归档、所有种类、发送、正在输入、已读标记、积压 |
| `docs/room-files.md` | 房间的文件：存储、消息文件、文件唤醒谁，以及放回文件的工具 |
| `docs/pictures.md` | 发送、接收和保存的图片，头像，以及智能体是否查看图片 |
| `docs/random-draw.md` | 公平抽签及其方法 |
| `docs/reminders.md` | 提醒和监视：wait 工具，以及 `bmo use` 如何运行它并唤醒智能体 |
| `docs/chat-mcp.md` | 接收器的工具：send 和 rooms 入口、调用的运行方式及其回合上下文 |
| `docs/store.md` | 接收器的数据库、表和设置键，以及它在数据根目录下的文件夹 |
| `docs/codes.md` | 接收器给出的所有拒绝、事实、固定文案和种类，及其字段和状态 |
| `docs/policy.md` | 策略文件以及接收器读取的键 |
| `docs/PERSONAL-DATA.md` | 任何受跟踪文件都不得包含的个人数据，以及保证这一点的检查 |
| `docs/checks.md` | 仓库检查：依赖、清单、目录结构、版本、韩文字符、个人数据和名称 |
| `docs/wording.md` | 本仓库英文文本禁止使用的术语，由测试平台的措辞检查把关 |
| `docs/inventory.md` | 本仓库从中拆分出来的单一仓库的每个成员：它在这里的位置、迁移它的任务，以及拥有未迁移成员的单元 |

## 个人数据

任何受跟踪的文件都不包含邮件地址、主目录路径、电话号码、令牌或私有网络地址。`docs/PERSONAL-DATA.md` 规定了
这条规则，测试平台的 `personal-data` 检查会读取每个受跟踪的文件来核查，报告文件、行和种类，但绝不报告值本身。

## 版本

- **整个仓库只有一个版本，** 即工作区的版本。`VERSION` 保存它，工作区的版本与之相同。
- **版本只在所有者宣布的发布时变动，** 如 bmo-chat 的 `docs/conventions.md` 「Versions」所述；提交、部署、修复或
  契约变更都不会改变版本，部署以其提交区分。

## 构建与测试

Cargo 通过测试平台的 Cargo 命令运行，该命令在唯一的共享构建目录中构建：

```sh
bmo-testbed check quick
bmo-testbed cargo check --workspace --all-targets
bmo-testbed cargo test --workspace
bmo-testbed cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

`bmo-testbed check quick` 在运行任何东西之前先执行四项读取源码树的检查：术语表中的术语、代码中的韩语、README
的目录表，以及清单中声明的依赖。人、智能体或机器所用的名称由仓库自己的测试检查，该测试从检出目录之外读取名称
列表。测试平台的 `personal-data` 检查会在每个受跟踪的文件中查找邮件地址、主目录路径、电话号码、令牌和私有网络
地址（`docs/PERSONAL-DATA.md`）。

使用数据库的测试会在 `BMO_TEST_POSTGRES` 指定的 PostgreSQL 服务器上创建自己的数据库，该变量由测试平台的测试
套件设置；未指定时使用本机的套接字。每个测试进程迁移一个模板 `receiver_rs_tmpl_<process>`，并给每个测试一份它的
副本 `receiver_rs_copy_<process>_<count>`，测试结束时删除该副本：先结束该角色自己在其上的会话，再以非强制方式
删除数据库，因此测试套件的角色不需要结束其他角色进程的权限，而副本未被删除的测试会失败。进程在创建第一个
数据库之前，会删除其角色所拥有的、对应进程已不在本机运行且没有任何客户端会话连接的模板和副本；其他名称一概
不动。
