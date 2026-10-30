# 沙箱、连接、订阅号池与用户绑定（草案）

**状态：** 沙箱 `run` 已在本仓库实现（`skilllite_sandbox::run`，命令 `agent-sandbox`）。号池、用户绑定和连接服务仍是草案，不在本仓库。

本方案不使用 Evotown 的 Doctor WebSocket、`job.assign`、引擎队列和 `agent-runs` 事件表。

四块：

| 块 | 做什么 |
|---|---|
| 订阅号池 | 这台电脑上已经登录好的订阅目录。每条记录一个订阅号 |
| 用户绑定 | 一个用户对应池里的一个订阅号 |
| 连接 | 电脑主动连出。手机只向连接服务订这次运行的 SSE |
| 沙箱 | `skilllite-sandbox` 的 `run`。一次运行一个沙箱 |

`run` 在 `skilllite-sandbox`。Agent Doctor 的桌面和 CLI 调用它。SkillLite 不依赖 Agent Doctor。`skilllite-agent`、进化引擎、`skilllite-commands`、桌面端 `skilllite-assistant` 不参与。技能执行的 `execute()` 保持断网、短超时和原来的 seccomp，与 `run` 互不调用。

## 号池放在私有仓库

公开仓库继续只做两件事。SkillLite 提供 `run` 这一个沙箱能力。Agent Doctor 继续是个人版：这个人登录自己的一个订阅号，自己的手机连回自己的电脑，没有第二格订阅号，也不能把这个号绑给别人。

订阅号池、用户绑定和连接服务放在一个不公开的仓库里。它调用 `skilllite-sandbox` 的 `run`，安装和诊断调用 Agent Doctor 已有的 `install` 和 `doctor`。不复制这两个仓库的代码，也不把号池加进它们的安装包。一个订阅号只属于把它登录进来的那个人。

Agent Doctor 的团队版按现有的编译期安装包保留，不改目录结构，也不纳入本方案。

## 订阅号池

池在本机。一条订阅号：

| 字段 | 对外可见 | 含义 |
|---|---|---|
| `account_id` | 是 | 稳定 id，如 `work`、`personal` |
| `runtime` | 是 | `claude` 或 `codex` |
| `ready` | 是 | 二进制存在，且订阅目录里有登录文件 |
| `busy` | 是 | 这个订阅号已经有一个沙箱 |
| 订阅目录 | 否 | `claude auth login` 留下的 `.claude`，或 `codex login` 留下的 `.codex` |
| `https_proxy` | 否 | 这个订阅号的出口代理 |

登录不在 `run` 里。人在宿主机上对那个目录执行一次官方登录。程序不读取、不保存、不转发 token，不打开 macOS 钥匙串。Codex 刷新会话后，宿主只把临时家目录里的普通文件 `auth.json` 写回订阅目录。

同一订阅号同时只有一个沙箱。不同订阅号可以并行。安装和诊断仍用已有的 `install`、`doctor`，用来把 `ready` 变成真。

## 用户绑定

一个用户绑定池里的一个订阅号。用户发来的这句话，就用他绑定的那个目录和代理。

个人版只有当前这个人的一条绑定，界面不展示号池。家庭节点才保管多个你自己的订阅号。同一台电脑同一时间只跑一个 `connect`。

一个订阅号只属于把它登录进来的那个人，不能改绑给别人。这个人已经有一个沙箱在跑时，后来的运行排队。

## 连接

家庭网络不对外开放端口。电脑主动连到连接服务。手机和电脑不直连。

```text
手机
    │  GET /runs/{run_id}/stream
    ▼
连接服务
    │  一条 WebSocket：运行请求下去，stream-json 逐行回来
    ▼
这台电脑的 connect（桌面或 CLI）
    │  用户 → 绑定的订阅号 → run
    ▼
沙箱
```

1. 手机把一句话交给连接服务，带上 `user_id` 和这次的 `run_id`。
2. 连接服务沿这台电脑已经建立的 WebSocket 送下去。
3. 电脑用该用户绑定的订阅号调用 `run`。stdout 为 `claude -p … --output-format stream-json --verbose`。
4. 每一行 JSON 送回连接服务。`sk-` 开头的密钥先写成 `[redacted]`。
5. 连接服务按 `run_id` 保存，并用 SSE 按顺序推给手机：正文、`tool_use`、`tool_result`。结束时发 `done`，带上退出状态。SSE 不承担登录和号池。

拟定中的一行：

```json
{
  "type": "run.stream",
  "run_id": "…",
  "user_id": "…",
  "account_id": "work",
  "line": { "type": "assistant", "message": {} }
}
```

断线重连后，还在跑的沙箱按原来的 `run_id` 继续推。连接服务不使用 Evotown 的任务队列。

## 沙箱

每次运行调用 `skilllite_sandbox::run`（命令行是 `skilllite agent-sandbox`）。订阅目录、工作目录、提示词和 `https_proxy` 都来自调用方，不从连接服务下发代理，也不把目录放进对外状态。`https_proxy` 可以不传。`--require-proxy` 或环境变量 `SKILLLITE_AGENT_SANDBOX_REQUIRE_PROXY=1` 打开后，没有代理就不会启动。传了代理时，macOS 上必须是本机地址，Seatbelt 把它写成 `localhost:<端口>`。Linux 上 nftables 用该 cgroup 在 `/sys/fs/cgroup` 下的完整路径做匹配，而不是只写叶子目录名。允许项装不上时，运行不会启动。

1. 建临时家目录。`--runtime claude`（默认）复制 `.claude`，`--runtime codex` 复制 `.codex`。原目录不挂进沙箱。Claude 结束时不写回。Codex 只把刷新后的 `auth.json` 写回。
2. 工作目录原位挂到 `/work`（macOS 上则作为工作目录）。结果留在这个目录里。同一工作目录同时只有一个 `run`。用完即删的只有临时家目录和里面的订阅副本。
3. 订阅目录加文件锁，与号池的 `busy` 一致。
4. 传了 `https_proxy` 时，在宿主机解析代理。沙箱内只允许连接这个 IP 和端口。其他 TCP、UDP 拒绝，包括局域网、公网，以及本机上除代理端口以外的地址。沙箱内不做 DNS。本地套接字允许。解析失败则这次运行不启动。没传代理时可以启动，出网不按代理收口。Linux 上这种不走代理的运行会在文件存在时只读挂载 `/etc/resolv.conf`、`/etc/nsswitch.conf` 和 `/etc/hosts`，进程才能解析域名。传了代理的运行不挂这些文件。开关 `--require-proxy`（或 `SKILLLITE_AGENT_SANDBOX_REQUIRE_PROXY=1`）打开后，没传代理则不启动。
5. 命令名先在宿主机解析成绝对路径，再传进沙箱。这个二进制所在目录会加进沙箱的 `PATH`。未指定其他命令时执行：

```text
claude -p "<提示词>" --output-format stream-json --verbose --permission-mode dontAsk
codex exec --json --dangerously-bypass-approvals-and-sandbox "<提示词>"
```

Codex 这条命令关掉它自己的沙箱。外层 bwrap 或 Seatbelt 仍然限制可写目录；传了代理时，出网仍然只到该代理。

6. 进程退出、连接服务取消或出错：杀掉进程组，Codex 写回 `auth.json`，删除临时家目录，释放锁，`busy` 改回假。

Linux 使用单独组装的 `bwrap`：用户、挂载、PID、UTS、IPC 命名空间；主机名用这次运行的 id；不使用网络命名空间，也不做独立网卡；只读挂 `/usr`、`/lib`、`/bin`、`/lib64`（存在才挂）；没传代理时，再只读挂 `/etc/resolv.conf`、`/etc/nsswitch.conf`、`/etc/hosts`（文件存在才挂）；临时家目录挂到 `/home/agent`；`/tmp` 用 tmpfs；不挂宿主家目录、Skilllite 配置目录和容器套接字；进程非 root。代理限制按这次进程的用户或 cgroup 写允许项。

macOS 使用单独的 Seatbelt profile 和 `sandbox-exec`。`HOME` 指向临时家目录。读范围保持放开，否则动态链接器起不来；写只允许临时家目录和工作目录。代理限制写在该 profile 的网络允许项里。没有 PID 命名空间，活动监视器里仍可能看见另一个订阅号的进程。订阅目录和出口 IP 按订阅号分开。

## 这次不做

Evotown 派活与事件表、独立网卡、按代理 IP 改时区、语言环境镜像、把订阅包装成 OpenAI 兼容接口、Windows、在桌面里管理号池、让桌面或 CLI 对外监听 SSE。Claude 和 Codex 共用同一套 `bwrap` 和 Seatbelt。

## 验收（实现之后）

- 用户绑定到一个 `ready` 的订阅号后，手机能从连接服务的 SSE 按顺序收到正文和工具调用，并在结束时收到 `done`。
- 手机的消息经连接服务到达这台电脑。电脑不需要对家庭网络开端口。
- 两个不同订阅号并行时，谁也读不到对方的临时家目录。Linux 上彼此的 `ps` 看不到对方。
- 运行结束后临时家目录不存在，订阅目录和工作目录仍在，该订阅号 `busy` 为假。
- 打开 `--require-proxy` 时，不带代理则这次运行不启动。传了代理时，直连代理以外的地址连接失败。
- 同一台电脑不同时运行桌面 `connect` 与 CLI `connect`。
- SkillLite 原有的技能执行测试仍然通过。
