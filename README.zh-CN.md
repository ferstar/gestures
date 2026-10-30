# Gestures (中文文档)

[English](./README.md) | 简体中文

基于 Linux libinput 的触控板手势工具，响应手势并触发命令或鼠标操作。直接调用 libinput API，而非解析 debug 文本输出，在 X11 与 Wayland 下均保证低延迟与高可靠性。

本 fork 专注于**极致性能的三指拖拽（Three-finger drag）**体验，针对 X11 与 Wayland 深度优化。技术背景参见 [上游讨论](https://github.com/riley-martin/gestures/discussions/6)。预编译二进制文件可在 [Releases 页面](https://github.com/ferstar/gestures/releases) 下载。

---

## 特性

- **多显示架构支持**：自动检测 X11 与 Wayland 环境。
- **高性能输入架构**：
  - **X11**：直连调用 `libxdo` C API，延迟极低。
  - **Wayland**：原生直写 `/dev/uinput` 虚拟指针设备；无需外部后台守护进程；自动降级到 `ydotool`。
  - **防 PID 耗尽**：命令执行由线程池统一管理（默认 4 个 worker）。
- **手势类型**：轻扫 Swipe（8 组方向 + any）、捏合 Pinch、按住 Hold。
- **三指拖拽**：模拟类似 macOS 的流畅三指拖拽，支持 `mouse-up-delay`（按键松开缓冲防断触）。
- **触控板指针设置**：支持独立进程级 libinput 加速度曲线（profile）与速度（speed）控制，支持热重载。
- **无缝热重载**：基于 IPC 实时重载配置，无需重启进程。
- **平稳退出**：SIGTERM / SIGINT 退出时安全清空队列、释放按键，防止出现“按键卡死”。

---

## 安装说明

### 依赖环境

**系统开发库：**
- `libudev-dev` / `libudev-devel`
- `libinput-dev` / `libinput-devel`
- `libxdo-dev` / `libxdo-devel`

**运行权限与服务：**
- **X11**：无需额外守护程序（直接链接 `libxdo`）。
- **Wayland**：当前用户需对 `/dev/uinput` 拥有读写权限（默认直写原生虚拟指针）。`ydotool` 与 `ydotoold` 仅用作降级备用或自定义命令调用。

### 通过 Cargo 安装

```bash
cargo install --git https://github.com/ferstar/gestures.git
```

### 源码编译

```bash
git clone https://github.com/ferstar/gestures
cd gestures
cargo build --release
sudo cp target/release/gestures /usr/local/bin/
```

### Nix Flakes

```nix
# flake.nix
{
  inputs.gestures.url = "github:ferstar/gestures";

  # 然后添加到 packages:
  # inputs.gestures.packages.${system}.gestures
}
```

---

## 配置指南

详细配置参数请参阅 [config.md](./config.md)。

### 快速初始化

```bash
# 生成默认配置文件（~/.config/gestures.kdl）
gestures generate-config

# 仅在终端预览配置内容
gestures generate-config --print

# 强制覆盖已有配置文件
gestures generate-config --force
```

### 配置示例

```kdl
// 三指拖拽（X11 与 Wayland 通用）
swipe direction="any" fingers=3 mouse-up-delay=500

// 四指横向轻扫切换工作区
swipe direction="w" fingers=4 end="hyprctl dispatch workspace e-1"
swipe direction="e" fingers=4 end="hyprctl dispatch workspace e+1"
```

### 触控板加速度配置

若需匹配桌面环境的指针速度曲线：

```kdl
touchpad accel-profile="adaptive" accel-speed=0.3
swipe direction="any" fingers=3 mouse-up-delay=500
```

- `accel-profile`：可选 `"adaptive"`（自适应）或 `"flat"`（线性）。
- `accel-speed`：范围 `[-1.0, 1.0]`，`0.0` 为硬件默认速度。
- 仅作用于本程序内的 libinput 上下文，不篡改桌面合成器的全局指针设定。
- 运行 `gestures reload` 即可在已有设备上即时生效。
- *注：旧版 `swipe` 下的 `acceleration` 参数已废弃并被忽略。*

---

## 运行与管理

### Systemd 用户服务（推荐）

```bash
# 1. 生成配置
gestures generate-config

# 2. 安装 systemd 服务文件
gestures install-service

# 3. 启用并启动服务
systemctl --user enable --now gestures.service
```

### 命令行运行

```bash
# 自动检测 X11 或 Wayland
gestures start

# 强制指定 Wayland 模式
gestures --wayland start

# 强制指定 X11 模式
gestures --x11 start

# 重载配置
gestures reload

# 预览服务文件模板
gestures install-service --print
```

---

## 性能设计细节

1. **正则预编译**：通过 `once_cell::Lazy` 仅在启动时单次编译。
2. **命令线程池**：固定 4 线程 Worker 队列，防极端连发导致 PID 耗尽。
3. **输入事件直驱拖拽**：直接鼠标移动不设固定帧率锁死，亚像素位移保留至下帧，低速拖拽细腻精准；高频移动合并（coalesce）进单帧交付。
4. **定时器异步延迟**：`mouse-up-delay` 采用非阻塞 Timer，抬手短暂停顿不会误中断选中。
5. **配置缓存**：1 秒手势检索缓存，避免高频遍历匹配。
6. **生命周期防护**：持有持久化 uinput 虚拟设备，退出或收到信号自动刷出剩余位移、释放鼠标按键、注销虚拟设备。

---

## 常见问题排查

### Wayland 下 CPU 占用

- 直接拖拽依据触控板硬件事件频率实时发送，高负载时队列自动合并移动事件。
- 自定义命令更新受 `ThrottleState::new(60)` 限制（每秒最多 60 次）。

### 三指拖拽无响应

**X11 环境：**
- 检查会话环境变量 `DISPLAY` 与 `XAUTHORITY` 是否对 systemd 用户服务可见。

**Wayland 环境：**
- 确保当前用户拥有 `/dev/uinput` 访问权限。
- 检查日志查看是否成功创建虚拟指针设备。如果降级为 `ydotool`，请确认 `ydotoold` 是否处于运行中。

### Wayland 权限不足（触控板或 uinput）

触控板原始事件读取与 `/dev/uinput` 创建均需要权限：

```bash
# 若 /dev/uinput 不存在则加载内核模块
sudo modprobe uinput

# 将当前用户加入 input 组（执行后需重新登录生效）
sudo usermod -aG input "$USER"
```

若 `/dev/uinput` 默认对 input 组无写权限，创建 `/etc/udev/rules.d/99-gestures-uinput.rules`：

```udev
KERNEL=="uinput", GROUP="input", MODE="0660"
```

重新加载 udev 规则：

```bash
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=misc --sysname-match=uinput
systemctl --user restart gestures.service
```

### X11 下报 `libxdo` 动态库缺失

若系统升级了 `xdotool` / `libxdo`（例如从 `.so.3` 升至 `.so.4`），原二进制可能链接失效。执行以下命令重新编译安装：

```bash
cargo install --path . --force
systemctl --user restart gestures
```

### 与桌面环境自带手势冲突

请在桌面设置中关闭系统的对应多指手势（如 GNOME、KDE 对应设置），避免双重响应。

---

## 同类项目对比

- [libinput-gestures](https://github.com/bulletmark/libinput-gestures) - 通过解析 `libinput debug-events` 输出工作
- [gebaar](https://github.com/Coffee2CodeNL/gebaar-libinput) - 仅支持 Swipe 轻扫
- [fusuma](https://github.com/iberianpig/fusuma) - 基于 Ruby 实现
