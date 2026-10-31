# Gestures (中文文档)

[English](./README.md) | 简体中文

基于 Linux libinput 的现代化触控板手势管理与图形配置工具。直接调用 libinput API 与原生 `/dev/uinput` 虚拟设备，针对 **Wayland 作为主打环境深度优化**（同时向下兼容 X11），带来媲美 macOS 的极致丝滑三指拖拽与零延迟手势体验。

预编译包（Debian `.deb`、Arch Linux `PKGBUILD`）与二进制文件可在 [Releases 页面](https://github.com/ferstar/gestures/releases) 下载。

---

## 核心特性

- **Wayland 优先**：原生直写 `/dev/uinput` 虚拟指针，零合成器干扰，性能极佳且告别延迟与卡顿；自动向下兼容 X11。
- **三指拖拽**：模拟媲美 macOS 原生手感的三指拖动，支持可配置的 `mouse-up-delay`（抬指缓冲防断触）。
- **原生图形控制面板**：极简流线型 GUI 配置面板（`gestures-panel`），支持中英双语切换、手势可视化管理、触控板指针加速度实时调参与自启动管理。
- **桌面原生自启动**：彻底摒弃传统 `systemd --user` 带来的权限与图形会话割裂痛点，直接通过标准 XDG Desktop Autostart 随桌面登录自启，天然拥有完整的图形会话与环境变量。
- **手势类型丰富**：轻扫 Swipe（8 组方向 + any）、捏合 Pinch（张开/收缩/旋转）、按住 Hold。
- **进程级触控板加速度**：独立配置加速曲线（`adaptive` 自适应 / `flat` 线性无加速）与倍率，不篡改桌面系统指针设定。
- **IPC 瞬间热重载**：执行 `gestures reload` 或在界面点击保存即可秒级生效，无需重启进程。
- **优雅退出无僵尸**：完善的后台进程收割与生命周期管理，停止时自动释放鼠标按键与虚拟设备。

---

## 安装方式

### 推荐：包管理器安装

#### Debian / Ubuntu
在 [Releases 页面](https://github.com/ferstar/gestures/releases) 下载一体化 `.deb` 包：
```bash
sudo dpkg -i gestures_1.1.0_amd64.deb
```
会自动安装守护进程、图形面板、桌面菜单项及全局自启动配置。

#### Arch Linux
使用仓库内置的 `PKGBUILD`：
```bash
cd packaging/arch
makepkg -si
```
或直接从源码生成 Arch 安装包：
```bash
./scripts/package-arch.sh
sudo pacman -U dist/gestures-*.pkg.tar.zst
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

### 通过 Cargo 安装

```bash
cargo install --git https://github.com/ferstar/gestures.git
```

### 本地源码编译

```bash
git clone https://github.com/ferstar/gestures
cd gestures

# 编译后台守护核心
cargo build --release
sudo cp target/release/gestures /usr/bin/

# 编译图形面板（需 Go 1.24+）
cd apps/panel
go build -o build/gestures-panel .
sudo cp build/gestures-panel /usr/bin/
```

---

## 运行与自启动

### 图形面板（推荐）
在应用程序菜单中搜索 **Gestures**（或手势控制），或直接在终端运行：
```bash
gestures-panel
```
在面板中可实时直观管理手势、调节加速度方案，并一键开启或关闭**桌面自启动**。

### 命令行自启动管理
将当前用户的自启动项写入 `~/.config/autostart/gestures.desktop`：
```bash
gestures install-autostart
```

### 常用命令

```bash
# 启动守护进程
gestures start

# 热重载配置
gestures reload

# 生成默认配置文件（~/.config/gestures.kdl）
gestures generate-config
```
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
- 自定义命令更新受 `ThrottleState::new(60)`（`crates/gestures/src/event_handler.rs`）限制（每秒最多 60 次）。

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
cargo install --path crates/gestures --force
systemctl --user restart gestures
```

### 与桌面环境自带手势冲突

请在桌面设置中关闭系统的对应多指手势（如 GNOME、KDE 对应设置），避免双重响应。

---

## 同类项目对比

- [libinput-gestures](https://github.com/bulletmark/libinput-gestures) - 通过解析 `libinput debug-events` 输出工作
- [gebaar](https://github.com/Coffee2CodeNL/gebaar-libinput) - 仅支持 Swipe 轻扫
- [fusuma](https://github.com/iberianpig/fusuma) - 基于 Ruby 实现
