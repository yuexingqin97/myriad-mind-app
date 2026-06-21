# Tauri + React 入门（给 UE 开发者）

> 这份文档不是完整教程，而是**给你建立心智模型**用的：看完能看懂本项目代码、能和我正常讨论。
> 全程用 Unreal Engine（蓝图 / C++ / UMG）的概念类比，并直接引用本项目真实代码（带文件路径）。
>
> 阅读时间：约 20 分钟。建议对着代码库一起看。

---

## 0. 30 秒先搞清楚：这俩分别是干嘛的

UE 的经典分工：**引擎核心用 C++ 写（`Source/Runtime/`，底层、高性能、跑在系统上），游戏逻辑用 C++ GamePlay / Blueprint 写（上层、迭代快）**。

本项目把这套架构搬到了桌面 App 上：

| 角色 | UE 里 | 本项目里 |
|------|-------|---------|
| 引擎核心（底层） | UE 的 C++ 引擎层（`Source/Runtime`） | **Rust**（`apps/desktop/src-tauri/`） |
| 脚本/UI 层（上层） | Blueprint / C++ GamePlay / UMG Widget | **React + TypeScript**（`apps/desktop/src/`） |
| 第三方工具库 | 插件（Wwise / FMOD / Niagara…） | **Python 脚本子进程**（`scripts/`） |

- **Tauri** = 那个"引擎 + 原生层"。它管窗口、文件系统、进程调度、打包。用 **Rust** 写。
- **React** = 写界面（UI）的脚本层。它负责"屏幕上长什么样、用户点了按钮怎么办"。

> 🎮 **一句话类比**：你以前用 Blueprint / C++ 给 UE 写 `AActor` 和 `UUserWidget` 做游戏；现在用 React 给 Tauri 写 UI 做桌面 App。Rust 干的就是 UE 引擎 C++ 干的活。

**为什么不用 Electron？** Electron 给每个 App 打包一整个 Chrome 浏览器（动辄 100MB+）；Tauri 用系统自带的 WebView，安装包小一个数量级（几 MB）。代价是不同系统的 WebView 略有差异，但本项目不需要极致兼容性，所以 Tauri 很合适。

---

## 1. 整体架构（对应本项目）

```
┌─────────────────────────────────────────────────────────────┐
│  React UI 层（WebView 里跑）  apps/desktop/src/              │
│  • 画界面、管交互、管前端状态                                   │
│  • 通过 invoke() 调后端 / 通过 listen() 收事件                  │
└───────────────▲──────────────────────────┬──────────────────┘
                │ 调用命令（请求/响应）        │ 监听事件（后端推送）
                │ invoke("read_config")       │ listen("pipeline-progress")
        ┌───────┴────────────────────────────▼───────────┐
        │       Tauri IPC 边界（invoke / emit 桥）        │
        └───────▲────────────────────────────▲───────────┘
                │                              │
┌───────────────┴──────────────────────────────┴──────────────┐
│  Rust 后端层  apps/desktop/src-tauri/src/                    │
│  • #[tauri::command] 暴露函数给前端                          │
│  • app.emit() 主动推送进度给前端                              │
│  • 调度 Python 子进程、读写文件、调 DeepSeek API              │
└───────────────────────────┬──────────────────────────────────┘
                            │ Command::new(python).output()
                            ▼
                   Python 脚本子进程（scripts/*.py）
                   下载 / FFmpeg / ASR / 关键帧
```

> 🎮 **类比**：把它当一个**多人游戏架构**。React UI 就是客户端，Rust 后端就是 **Listen Server / Dedicated Server**，IPC 就是它们之间的 **Replication / RPC**。`invoke` 是客户端发 **Server RPC**（请求-响应），`emit/listen` 是服务器 **NetMulticast** 广播给所有客户端（推流，像服务器推同步事件）。

---

## 2. React 篇（UI 脚本层）

### 2.1 组件（Component）= UI 的 Widget Blueprint

在 UE 里你做一个 UI，会做一个 **Widget Blueprint（`UUserWidget`）**；做一个可复用对象，会做 **Actor Blueprint**。React 的**组件**就是 Web 版的这些——一段可复用的 UI + 逻辑。

本项目入口在 `apps/desktop/src/App.tsx`：

```tsx
// App.tsx —— 这就是一个组件（函数组件）
function App() {
  const { view, setView, config } = useConfig();
  useTheme();

  return (
    <div className="app-root">
      <Sidebar activeView={view} onNavigate={setView} />
      <main className="main-content">
        {view === "input" && <InputView config={config} />}
        {view === "dashboard" && <DashboardView />}
        {view === "settings" && <SettingsView config={config} onSave={saveConfig} />}
      </main>
    </div>
  );
}
```

**几个要适应的点：**

1. **JSX 语法**：`return (<div>...</div>)` 看着像 HTML，其实是 JavaScript（所以文件后缀是 `.tsx`）。你在 JS 里直接写"标签"，编译器把它转成函数调用。
2. **大括号 `{}` 里塞 JS 表达式**：`{view === "input" && <InputView/>}` 是"如果 view 是 input 就渲染 InputView，否则什么都不渲染"。这就是个 JS 三元/与表达式。
3. **组件就是函数**：函数名大写开头（`App`、`InputView`），返回一段 JSX。**没有 class、没有 `this`、没有 New**——这点和 UE 完全不同：UE 的 `AActor` / `UUserWidget` 都是 `UCLASS`（面向对象、有实例、有 `this->`），React 函数组件不是。

> 🎮 **类比**：一个组件 ≈ 一个 Widget Blueprint。`<InputView config={config} />` 就像你在 UMG 里 `CreateWidget` 后给它的某个 `UPROPERTY` 赋值，或者在 Details Panel（细节面板）里拖引用。

### 2.2 Props vs State = 外部参数 vs 内部变量

这是 React 最核心的概念，对应 UE 的 `UPROPERTY`：

| React | UE 对应 | 能不能改 |
|-------|---------|---------|
| **Props**（`{ config }: InputViewProps`） | `UPROPERTY(EditAnywhere)`，由**父控件/创建者**在 Details 里塞进来的引用 | ❌ 只读，别改 |
| **State**（`useState`） | 组件自己内部的 `UPROPERTY` 私有变量（配合数据绑定让 UI 刷新） | ✅ 能改，改了会触发重新渲染 |

看 `apps/desktop/src/hooks/useConfig.ts`：

```tsx
const [view, setView] = useState<"input" | "dashboard" | "settings">("input");
const [config, setConfig] = useState<MyriadMindConfig>(DEFAULT_CONFIG);
```

- `useState(初始值)` 返回一对：`[当前值, 修改函数]`。
- 你**永远不能直接 `view = "xxx"`**，必须调 `setView("xxx")`。因为 React 要靠"你调了 set"才知道数据变了、需要重新渲染。

> 🎮 **类比**：Props 像 `UPROPERTY(EditAnywhere)`——在 Details Panel / 父级设置的，运行时你不该乱改。State 像组件内部 `UPROPERTY(BlueprintReadWrite)` 私有变量——你自己管，但要通过 setter/绑定让 UI 知道"值变了，刷新一下"（这点 React 是全自动的）。

### 2.3 副作用与生命周期 = BeginPlay / Tick / EndPlay

UE 有 `BeginPlay()` `Tick()` `EndPlay()` 这些生命周期函数（UMG 是 `NativeConstruct()` / `NativeTick()` / `NativeDestruct()`）。React 用 **`useEffect`** 一个 Hook 覆盖大部分场景：

```tsx
// useConfig.ts —— 启动时加载配置
useEffect(() => {
  (async () => {
    if (await isTauri()) {
      const first = await api.isFirstLaunch();
      if (first) { setFirstLaunch(true); setView("settings"); }
      // ...
    }
  })();
}, []); // ← 第二个参数是"依赖数组"
```

**读法：**

- `[]` 空数组 = **只在组件挂载时跑一次**（≈ UE 的 `BeginPlay` / UMG 的 `NativeConstruct`）。
- `[a, b]` = 当 `a` 或 `b` 变化时再跑（≈ 响应式，比 UE 的 Tick 更聪明，不无脑每帧跑）。
- 不写第二个参数 = 每次渲染都跑（⚠️ 一般是 bug）。
- **return 一个函数** = 清理逻辑（≈ UE 的 `EndPlay` / `NativeDestruct`，比如取消事件监听）。

> 🎮 **关键差异**：UE 是每帧自动调 `Tick(deltaTime)`；**React 没有"每帧 Tick"概念**。React 是**事件驱动 + 声明式**：只有当 state 变化、或父组件重渲染时，组件才会重新算一遍。所以你不会写"每帧 Tick 检查进度然后更新 UI"，而是"后端推一个事件 → 我 setState → React 自动刷新 UI"。

### 2.4 自定义 Hook = 复用逻辑的 UActorComponent

UE 里你会把公共逻辑抽成一个 **`UActorComponent`**，挂在多个 Actor/Widget 上复用（比如血量组件、背包组件）。React 里把"带状态的复用逻辑"抽成 **自定义 Hook**，名字以 `use` 开头：

```tsx
// 我写一行 const { view, setView, config } = useConfig();
// 就拿到了一整套"配置 + 视图切换"的逻辑
export function useConfig(): UseConfigResult {
  const [view, setView] = useState(...);
  const [config, setConfig] = useState(...);
  // ...各种逻辑...
  return { config, view, setView, /* ... */ };
}
```

本项目有三个 Hook（在 `apps/desktop/src/hooks/`）：

- `useConfig.ts` —— 配置读写 + 视图切换
- `usePipeline.ts` —— 炼化管线状态（输入、进度、日志、流式输出）
- `useTheme.ts` —— 主题切换

> 🎮 **类比**：Hook 就像一个可以挂到任何 Widget/Actor 上的 `UActorComponent` 逻辑插件。你想让哪个组件有"配置管理"能力，调一下 `useConfig()` 就行，不用改继承链。

### 2.5 其他常用 Hook（速查）

| Hook | 干嘛的 | UE 类比 |
|------|-------|---------|
| `useState` | 存可变状态 | `UPROPERTY` 私有字段 |
| `useEffect` | 副作用/生命周期 | `BeginPlay`/`Tick`/`EndPlay` |
| `useCallback` | 缓存函数引用 | 避免子控件无谓重绘 |
| `useMemo` | 缓存计算结果 | 缓存昂贵计算，别每帧重算 |
| `useRef` | 存"不触发渲染"的值 | 普通指针 / 句柄（`TWeakObjectPtr` 之类） |

> ⚠️ **规则铁律**：Hook 只能在组件/Hook 函数的**顶层**调用，不能放进 `if`、`for`、嵌套函数里。就像 UE 的 `UPROPERTY`/`UFUNCTION` 不能写在函数局部作用域里——它们是"类声明级"的东西。

### 2.6 声明式渲染 = "数据长啥样，UI 就长啥样"

这是 React 和命令式 UI 最大的思维差异：

- **命令式**（UMG 常规用法）：进度到 50% → 你写 `ProgressBar->SetPercent(0.5f); Text->SetText(FText("50%"));`
- **声明式**（React）：你只维护一个 `progress` 变量 → JSX 里写 `进度：{progress}%` → `setProgress(50)` 后 React **自动**帮你算出新 UI 并应用。

你**不手动调 SetText / SetVisibility**，你只改数据，React 帮你算 DOM diff。

> 🎮 **类比**：UMG 默认是**命令式**的（你手动 `SetText`/`SetVisibility`）。React 的"数据驱动自动刷新"在 UE 里最接近 **UE5 的 MVVM 系统**（`UMG` + `UMVVMViewModel` + `FieldNotify` + 数据绑定）：你改 ViewModel 里的属性，绑定的 Widget 自动刷新，不用手动调 setter。如果没用过 MVVM，就把它理解成 **"UMG + 自动同步刷新"**——你只管改数据，控件自己重绘。

---

## 3. Tauri 篇（Rust 引擎层）

### 3.1 命令（Command）= RPC，后端暴露给前端的函数

前端（React/WebView）跑在沙箱里，默认碰不到本地文件系统、不能起进程。Rust 后端通过 **`#[tauri::command]`** 宏把一个普通 Rust 函数"注册"成前端可调用的远程函数。

后端定义（`apps/desktop/src-tauri/src/commands/config.rs`）：

```rust
#[tauri::command]
pub fn get_config_info() -> ConfigFileInfo {
    let path = config_file();
    ConfigFileInfo {
        exists: path.exists(),
        is_first_launch: !path.exists(),
        path: path.to_string_lossy().to_string(),
    }
}
```

**必须注册**到 handler 列表里，否则前端调不到（`apps/desktop/src-tauri/src/lib.rs`）：

```rust
tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![
        health_check,
        get_version,
        get_config_info,
        read_config,
        write_config,
        execute_pipeline,
        run_mind_task,
        // ... 几十个命令
    ])
    .run(tauri::generate_context!())
```

前端调用（`apps/desktop/src/api.ts`）：

```ts
import { invoke } from "@tauri-apps/api/core";

export async function getConfigInfo(): Promise<ConfigInfo> {
  return invoke("get_config_info") as Promise<ConfigInfo>;
  //     ^^^^^^ 函数名字符串必须和 Rust 里的一模一样
}
```

> 🎮 **类比**：就像 UE 里你用 `UFUNCTION(Server / Client / NetMulticast)` 标记一个 RPC 函数，或用 `UFUNCTION(BlueprintCallable)` 把 C++ 函数暴露给蓝图。这里 Rust 函数是"服务器实现"，React 是"调用方"。`generate_handler!` 那一长串就是"把这些函数注册进 RPC 表"。

**数据怎么传？** Rust 结构体加 `#[derive(Serialize, Deserialize)]`，自动序列化成 JSON 过 IPC；前端用 TypeScript `interface` 描述同一形状。两边靠"约定 JSON 结构"对接，就像 UE 的 `USTRUCT` + 属性复制（Replication）序列化，只不过这里用 JSON 而非 UE 的二进制。

### 3.2 事件（Events）= 多播委托 / Event Dispatcher，后端主动推送

Command 是"前端问、后端答"（请求-响应）。但炼化一个视频要好几分钟，前端需要**实时知道进度**——这时用**事件**：后端干着活，隔一会儿往外"广播"一条进度，前端挂着监听收。

后端推送（`apps/desktop/src-tauri/src/commands/pipeline.rs`）：

```rust
use tauri::{AppHandle, Emitter};

// app.emit("事件名", 数据) —— 广播给所有监听者
emit_progress(app, "metadata", "📋 获取视频信息", 8.0, "running", None);
emit_progress(app, "deps", "环境检查通过", 10.0, "completed", None);
emit_progress(app, "cleanup", "清理完成", 98.0, "completed", None);
```

前端监听（`apps/desktop/src/api.ts`）：

```ts
import { listen } from "@tauri-apps/api/event";

// 返回一个"取消监听"函数，组件卸载时要调用
export function listenPipelineProgress(onEvent: (e: unknown) => void) {
  listen("pipeline-progress", (e) => onEvent(e.payload));
}
```

> 🎮 **类比**：就是 UE 的**多播委托**（`DECLARE_DYNAMIC_MULTICAST_DELEGATE` / 蓝图里的 **Event Dispatcher**）。Rust `emit` = `Delegate.Broadcast(data)`，React `listen` = `Delegate.AddDynamic(handler)`，组件销毁时取消订阅 = `RemoveDynamic` / `RemoveDynamic`。

**本项目两条主要事件流：**

| 事件名 | 方向 | 用途 |
|--------|------|------|
| `pipeline-progress` | Rust → React | 管线各步骤进度（百分比、状态、描述） |
| `mind-stream` | Rust → React | DeepSeek 流式生成的笔记内容（逐字推送） |

### 3.3 配置文件 `tauri.conf.json`

Tauri 的"项目设置"——窗口大小、打包目标、要打进安装包的资源、开发服务器地址。类比 UE 的 **Project Settings**（`DefaultEngine.ini` 那一堆）。

本项目关键配置（`apps/desktop/src-tauri/tauri.conf.json`）：

```jsonc
{
  "productName": "myriad-mind-desktop",
  "build": {
    "devUrl": "http://localhost:1420",       // dev 时 WebView 加载这个地址
    "beforeDevCommand": "pnpm dev",          // 启动 Tauri 前先跑前端 dev server
    "beforeBuildCommand": "pnpm build"       // 打包前先 build 前端
  },
  "app": {
    "windows": [{ "title": "大衍决 · ...", "width": 1200, "height": 800 }]
  },
  "bundle": {
    "resources": ["../../../scripts/*.py", "prompts/**/*.md"]  // 打进安装包
  }
}
```

> 📌 **改了提示词不用重编译**：`prompts/**/*.md` 作为资源打进包，运行时由 `PromptManager` 用 minijinja 渲染。改 `.md` 即可，不动 Rust。

### 3.4 权限（Capabilities）—— Tauri 2.x 的新东西

Tauri 2.x 默认**前端啥原生能力都没有**（读文件、开对话框、调系统命令……全锁死）。要用，必须在 `src-tauri/capabilities/` 里显式授权。这跟手机 App 申请权限一个思路（UE 打包 Android/iOS 时也要配 `AndroidManifest.xml` / `Info.plist` 权限）。

> 🎮 **类比**：像 UE 手游打包时的**权限配置**，或 iOS 的 `Info.plist`。默认不给，按需申请，安全。

---

## 4. 前后端怎么串起来（本项目最值得学的封装）

本项目 `apps/desktop/src/api.ts` 做了一层很巧妙的封装，**值得你重点看**：

```ts
// 检测是否在 Tauri 里运行；不在就降级成 mock（浏览器里也能跑 UI）
let tauriInvoke = null;
async function ensureTauri() {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    tauriInvoke = invoke;
    return true;
  } catch {
    console.warn("Not running inside Tauri — using mock mode");
    return false;
  }
}

export async function detectPython(): Promise<DepResult> {
  if (await ensureTauri()) return tauriInvoke!("detect_python");  // 真机
  return { name: "Python", found: false };                        // 浏览器降级
}
```

**为什么这么设计？** 这样你 `pnpm dev`（纯浏览器，不启 Tauri）也能调 UI，所有原生调用自动降级成假数据。前端组件**完全不用关心**自己跑在哪——这套封装让你能脱离 Rust 快速迭代 UI。

**一次完整调用的往返（以"点炼化按钮"为例）：**

```
1. 用户点按钮（React 事件 onClick）
2. usePipeline.submit() 被调用
3. api.invoke("execute_pipeline", { ...参数 })  →  跨 IPC
4. Rust execute_pipeline 命令执行，调度 Python 子进程
5. Rust 每完成一步 → app.emit("pipeline-progress", {...})  →  跨 IPC
6. 前端 listen("pipeline-progress") 回调 → setProgress(...) → setLogs(...)
7. React 检测到 state 变化 → 自动重新渲染进度条和日志
```

> 🎮 **类比**（UE 多人游戏）：玩家按技能键 → 客户端发 **Server RPC** → 服务器执行技能逻辑 → 服务器 **NetMulticast** 广播伤害/特效给所有客户端 → 客户端订阅到 → 更新血条 Widget。完全是同一套模式，只是这里 UI 客户端 = React，游戏服务器 = Rust。

---

## 5. 开发流程速查（每天都会用）

```bash
pnpm dev            # = pnpm dev:desktop，启动 Tauri + 前端，打开桌面窗口（开发主力）
pnpm dev:desktop:web # 只启前端 dev server（浏览器里调 UI，走 mock，不起 Rust）
pnpm build          # 全量构建（core + ui + desktop）
pnpm typecheck      # TypeScript 类型检查（写完前端跑一下）

# 桌面端独有
pnpm tauri dev      # 同 dev:desktop
pnpm tauri build    # 打包出 .exe / .msi 安装包
```

**两层开发的节奏（重点理解）：**

| 你改了什么 | 要不要重新编译 | 生效方式 |
|-----------|--------------|---------|
| React / TS 代码（`.tsx`/`.ts`） | ❌ 不用 | **HMR 热更新**，保存即刷新（秒级） |
| CSS | ❌ 不用 | HMR |
| 提示词 `.md`（`src-tauri/prompts/`） | ❌ 不用 | dev 重启 App 即生效 |
| Rust 代码（`.rs`） | ✅ 要重编译 | Rust 编译慢（几十秒），改 Rust 要耐心 |

> 📌 **改 Rust 是本项目最慢的环节**（比 UE 的 Live Coding 慢，更接近 UE 全量编译）。尽量把逻辑放前端（`packages/core` 纯逻辑）或提示词里，减少动 Rust 的次数。

---

## 6. UE 开发者最容易踩的坑

1. **❌ 在渲染过程里直接改 state → 死循环**
   ```tsx
   function Bad() {
     const [x, setX] = useState(0);
     setX(x + 1); // 🚫 组件渲染时改 state → 触发重渲染 → 又改 state → 无限循环
   }
   ```
   相当于在 UE 的 `Tick` / Paint 回调里反复触发重绘。要改 state，放进**事件处理函数**或 `useEffect` 里。

2. **❌ 以为有"每帧 Tick"**：React 不是 Tick 循环。别写 `while(true)` 轮询进度——用事件 (`listen`) 或定时器（`setInterval` ≈ UE 的 `SetTimer`）。

3. **❌ 找不到 `this`**：函数组件没有实例，没有 `this`。UE 那套 `this->`、`GetOwner()`、`GetWorld()` 在这里都不存在。

4. **⚠️ 渲染列表要给 `key`**：像给每个 Actor 一个唯一 `GUID` / 名字，让引擎能追踪。
   ```tsx
   {videos.map(v => <VideoCard key={v.id} video={v} />)}  // ✅
   ```

5. **⚠️ 异步用 async/await，不是协程**：UE 的 latent action / `AsyncTask` / `SetTimer` / 蓝图 `Delay` 节点，在这里是 `await new Promise(r => setTimeout(r, 1000))` 和 `async/await`。没有 UE 那种 `LatentInfo` 协程，就是 JS 的 Promise。

6. **⚠️ Rust 的所有权/借用**：Rust 比 C++ 还严，编译器会拒绝很多"看起来没问题"的代码（生命周期、借用检查）。这个等你要写 Rust 时再细究，前期读代码能看懂即可。

7. **⚠️ TypeScript 类型**：比 C++ 模板还繁琐一点点。`interface` 定义数据形状，参数/返回值都要标类型。本项目 `@/` 是路径别名（指 `apps/desktop/src/`），`@myriad-mind/core` 是共享逻辑包。

---

## 7. 心智模型一句话总结

> 把它当一个**"前端 React 当客户端、后端 Rust 当服务器、IPC 当网络协议、Python 当中间件"**的实时应用就行。你所有 UE 多人游戏里"客户端-服务器-RPC-多播广播"的经验都能直接迁移过来。差别只在：UI 是**声明式**的（改数据自动刷新，类似 UE5 MVVM），不是 UMG 那种命令式每帧手动更新。

---

## 8. 建议的学习路径

1. **先读懂本文档**，建立全局观。
2. **打开 `apps/desktop/src/App.tsx` → `useConfig.ts` → `api.ts`**，顺着这三条线把"前端启动→加载配置"这条链路走通。
3. **读 `lib.rs` 的 `generate_handler!` 列表**，扫一遍后端暴露了哪些命令（= 这个 App 能干什么）。
4. **挑一个简单命令**（如 `get_config_info`）从 Rust 定义 → 注册 → 前端 `api.ts` → 组件调用，完整走一遍。
5. **想上手改**：从改一个 React 组件的 UI 开始（纯前端，HMR 即时见效，最有成就感）。

### 官方延伸阅读（需要时查）
- React 中文文档：https://zh-hans.react.dev/ （**重点看：组件、State、Effect、响应式**）
- Tauri 2.x 官方文档：https://tauri.app/ （重点看：Commands、Events、Capabilities）
- TypeScript 速成：https://www.typescriptlang.org/zh/docs/

---

*本文档对应代码库截至 2026-06 的状态。随项目演进，引用的文件路径/函数可能变化，以实际代码为准。*
