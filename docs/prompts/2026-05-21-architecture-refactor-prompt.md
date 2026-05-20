# MinnowSnap 架构重构提示词

你是负责 MinnowSnap 的高级 Rust 桌面应用架构师。请在 `E:\Projects\Rust\MinnowSnap` 中执行一次以可维护性为核心的架构重构。本轮只处理一个已认领的 Beads issue 或一个明确的 plan phase；允许移动、删除、合并内部代码，但每个改动都必须服务于该 issue 的验收标准。不得顺手重排无关模块、目录或 API；跨出当前 phase 的发现只记录为 Beads follow-up。默认保留当前用户可见行为，除非 active spec 明确说明要改变。

## 项目上下文

- 项目是 Rust 2024 workspace，主要 crate 是 `crates/minnow-app`，二进制名为 `MinnowSnap`。
- 当前 UI 技术栈是 GPUI / `gpui-component`，不是 Tauri。旧 Qt/CXX-Qt 代码在 `legacy/qt`，只作为历史归档，不能重新进入 active build。
- 主要源码边界是 `app`、`platform`、`services`、`ui/features`。
- 已有 active 架构文档：
  - `docs/specs/2026-05-18-architecture-cleanup-spec.md`
  - `docs/plans/2026-05-18-architecture-cleanup-plan.md`
- 已归档历史文档：
  - `docs/specs/archive/2026-05-17-conservative-merge-refactor-design.md`
- 项目使用 Beads 追踪工作。先运行 `bd prime`，用 `bd` 创建、认领、更新、关闭任务；不要用 TodoWrite、TaskCreate 或 markdown TODO 列表做任务状态追踪。
- Shell 默认使用 Git Bash。文件操作必须使用非交互参数，例如 `cp -f`、`mv -f`、`rm -f`、`rm -rf`。

## 目标

把当前 issue 覆盖的 MinnowSnap 区域整理成职责清晰、真实来源唯一、容易测试和维护的现代 Rust 代码：

- 目录结构表达真实领域边界，而不是历史增长痕迹。
- 对本 phase 涉及的领域，明确“当前重复来源 -> 目标唯一来源 -> 暂留适配层理由”；未列入该表的领域不得主动重构。
- 删除有证据证明无调用、无语义、无外部边界价值的死代码、重复定义、无意义 re-export、零实现 wrapper、只为旧路径存在的兼容层。
- 优先使用已有依赖和主流、维护良好的库。需要新增或升级库时，先确认它确实能减少复杂度，并通过 Context7 获取当前文档后再改。
- 使用现代 Rust 写法：清晰所有权、小 API 面、crate-private 优先、显式错误类型、少 clone、少全局可变状态扩散。

## 必须先核对 spec 和 plan

开始改代码前，先完成任务和文档核对：

1. 运行 `bd ready`，选择一个 architecture cleanup 相关 issue，并用 `bd update <id> --claim` 认领。
2. 读取现有 active spec/plan；不要新建第二套 active 架构文档。
3. 只有当现有 active spec/plan 已不能描述当前 phase 时才更新它们；归档仅限被明确替换的旧文档。
4. 如果需要更新 spec，必须包含：
   - 当前架构问题清单。
   - 目标模块边界。
   - 单一真实来源表。
   - 删除/合并规则。
   - 不做事项。
   - 验收标准。
5. 如果需要更新 plan，必须包含：
   - 分阶段执行顺序。
   - 每阶段目标文件或模块。
   - 每阶段风险与回滚点。
   - 每阶段验证命令。
6. 如果 spec/plan 仍准确，在 Beads notes 中记录 `spec/plan reviewed, no update needed`。

## 当前优先重构靶点

从项目现状出发，这些是 architecture cleanup 的背景靶点。执行时只处理已认领 issue 对应的区域；其他靶点只能作为背景或 follow-up：

- Public API 边界：`minnow_app::app` 应是 crate root 的主要 public facade；`platform`、`services`、`ui` 默认 crate-private。
- App composition：`app::composition` 负责应用装配和跨模块 wiring，避免业务策略散落在 platform 或 UI support。
- Settings：配置持久化只属于 `services::settings`；UI preferences 只做表单状态、校验和 dispatch。
- i18n：用户可见文案只属于 `services::i18n` 和 locale YAML。
- Hotkeys：统一 `services/settings.rs`、`services/hotkeys.rs`、`platform/hotkey.rs`、`ui/features/preferences/state/shortcuts.rs` 的职责边界。
- Appearance / language / font：统一 `services/settings.rs`、`ui/support/appearance.rs`、`ui/support/locale.rs`、`ui/features/preferences/state/general.rs` 的所有权。
- OCR / pin：统一 `services/ocr/*`、`ui/features/preferences/state/ocr.rs`、`ui/features/pin/view/ocr_geometry.rs`、`ui/features/pin/view/ocr_text.rs` 的数据和渲染边界。
- Overlay annotation：拆分过大的 annotation engine，把文档 mutation、hit testing、render prep 分离。
- Capture / image pipeline：保持截图、OCR、长截图、stitching 等重计算逻辑在 services；UI 只编排窗口状态和渲染。
- Platform adapters：`platform` 只做 OS/GPUI 适配，产品策略回到 `app` 或 `services`。
- Legacy Qt：继续隔离在 `legacy/qt`，active crate、测试、CI 不得依赖它。

## 重构规则

- 删除优先于保留兼容层。只要没有真实外部消费者，就不要为了旧路径保留 shim。
- 删除 re-export、wrapper 或 shim 前必须记录证据：调用点搜索结果、是否属于 documented facade、删除后的编译/测试结果。属于 `minnow_app::app` 公共边界的 re-export 不按“零价值”删除。
- 合并只转发、只改名、只包一层的函数或模块。
- 拆分同时承担多种职责的大文件，但不要把紧密协作的 30 行逻辑拆成新抽象。
- 每次移动代码都同步收紧可见性，避免 `pub` 扩散。
- 不保留重复常量、重复枚举、重复配置模型、重复转换函数。
- 不为了“架构感”引入空 trait、空 service、manager/facade 包装层。
- 本提示词禁止框架、运行时、UI toolkit 迁移。若发现需要迁移，只能创建 Beads follow-up，不在本轮执行。
- 不把计划文件当任务状态源；任务状态只记录在 Beads。

## 验证要求

每个阶段至少运行该 issue plan 中列出的验证命令。关闭 Beads issue 的前提是 mandatory gate 全部通过；若任何 mandatory gate 失败，不得关闭 issue，必须用 `bd update <id> --notes` 记录失败命令、错误摘要、影响范围和下一步。

最终候选验证命令：

```bash
cargo fmt --check
cargo check -p minnow-app
cargo test -p minnow-app
cargo test -p minnow-app --test module_layout_smoke
cargo clippy -p minnow-app -- -W clippy::all
python scripts/check_no_qt_runtime_deps.py
```

如果安装了 `cargo machete`，还要运行依赖清理检查；如果未安装，记录为明确 blocker，不要假装通过。

## 完成标准

- active spec/plan 已核对；仅当本 phase 改变架构决策时更新。未变更时已在 Beads notes 中记录 `spec/plan reviewed, no update needed`。
- Beads issue 已更新或关闭，剩余工作拆成子 issue。
- 模块边界由具体检查锁定：`module_layout_smoke` 覆盖 crate-root public modules；本 phase 新增或更新测试断言目标边界；`rg` 检查无 active build 引用 `legacy/qt`。
- 重复来源和零价值封装被删除，或在 Beads issue 中用证据解释为什么暂留。
- active build 不依赖 `legacy/qt`。
- 所有 mandatory 质量门禁通过；失败时 issue 保持 open/in_progress，且失败原因、影响范围、下一步已记录。
- 会话结束前确认 `git status`，只 stage 本 issue 相关文件，提交后执行 `git pull --rebase`、`bd dolt push`、`git push`，最后 `git status` 必须显示无未提交任务改动且 branch up to date with origin。
