# git-knife 🔪

*像编辑表格一样，把 git 历史整理成你想要的样子——每条提交的 message、作者和日期，都能直接改。*

[English](./README.md) · **中文**

![git-knife 截图](docs/screenshot.png)

一个干净的桌面 GUI，用于直接编辑 git 提交的元数据——**message、作者日期、提交者日期、作者姓名/邮箱**。

已有的 GUI（GitKraken、Sublime Merge、Fork、lazygit）在改 message、重排提交方面做得很成熟，但把提交**日期**视为事实上的不可变，也不提供任意提交的提交者日期 / 作者身份的修改入口。而那些真正能改这些元数据的工具（`git-filter-repo`、`git rebase` 环境变量技巧、`git commit-tree`）又没有 GUI。git-knife 正是补上这个空缺：一个干净 GUI，安全地批量编辑每个字段。

它从不重新实现 git——只调用系统的 `git` CLI，用 `git commit-tree` 重建提交，并复用每条提交的原始 tree，因此**文件内容可证明地永远不会被改动**。

## 与其他工具的对比

| 工具 | 干净 GUI | 改 message | 重排 / 合并 / 删除 | 改**作者日期** | 改**提交者日期** | 改作者/邮箱 | 批量查找替换（正则） |
|------|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| **git-knife** | ✅ | ✅ | 🚧 规划中 | ✅ | ✅ | ✅ | ✅ |
| GitKraken | ✅ | ✅ | ✅ | ⚠️ 仅 amend | ❌ | ⚠️ | ❌ |
| Sublime Merge | ✅ | ✅ | ✅ | ❌ | ❌ | ⚠️ amend | ❌ |
| Fork | ✅ | ✅ | ✅ | ❌ | ❌ | ⚠️ | ❌ |
| SmartGit | ✅ | ✅ | ✅ | ⚠️ | ❌ | ✅ | ❌ |
| git-cola | ◐ | ✅ | ✅ | ✅ | ❌ | ✅ | ❌ |
| lazygit (TUI) | ◐ TUI | ✅ | ✅ | ❌ | ❌ | ⚠️ | ❌ |
| git-filter-repo (CLI) | ❌ | ✅ | 通过回调 | ✅ | ✅ | ✅ | ✅ |

图例：✅ 一等支持 · ⚠️ 可以做到，但别扭/受限 · ◐ 界面老旧或为终端 · ❌ 不支持 · 🚧 规划中。

成熟的 GUI 在改 message 和重排方面做得很好，但把提交**日期**——尤其是提交者日期——视为事实上的不可变，而且没有一个提供对作者身份的批量正则替换。那些能改这些元数据的工具又没有 GUI。git-knife 正是两者的交集：一个能安全地批量编辑每个字段的干净 GUI。

## 当前状态（MVP）

- ✅ 打开仓库并选择**任意本地分支**来查看/编辑——直接按 ref 操作，无需 checkout，你的工作区永远不会被动
- ✅ 编辑 message / 作者姓名+邮箱 / 作者日期 / 提交者日期 / 提交者姓名+邮箱
- ✅ 在这些文本字段上做批量查找替换，支持字面量或正则（修复到处写错的邮箱非常好用）
- ✅ 应用前预览每一项改动
- ✅ 每次重写前自动保存备份 ref，支持一键恢复
- ✅ 重写会碰到已推送历史时给出警告
- ✅ 感知签名提交——给签名提交打标、警告重写会移除签名，并可用你的密钥**重新签名**
- ✅ 支持跨合并重写——重建完整提交图，保留每个合并的父提交
- ⛔ 尚未实现：重排 / 合并 / 删除提交、暂存区 / 分支 / 远程管理

## 环境要求

- **git**（2.x）
- **Node.js** + **pnpm**（`corepack enable pnpm`，或 `npm i -g pnpm`）
- **Rust**（stable）——从 <https://rustup.rs> 安装
- Tauri v2 的 Linux 系统依赖：`webkit2gtk-4.1`、`libgtk-3`、`libayatana-appindicator3`、`librsvg2`
  （Debian/Ubuntu：`sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`）

## 运行（开发）

```bash
pnpm install
pnpm tauri dev
```

首次 `cargo` 构建会下载并编译 Tauri 相关 crate（需要几分钟）。

## 打包

```bash
pnpm tauri build
```

> 打包（`tauri build`）需要应用图标。图标已提交在 `src-tauri/icons/` 下；如需从任意方形 PNG 重新生成，用
> `pnpm tauri icon path/to/icon.png`。开发运行不需要它们。

### 自动发布（GitHub Actions）

`.github/workflows/release.yml` 使用 [`tauri-action`](https://github.com/tauri-apps/tauri-action) 为 **macOS、Linux、Windows** 构建原生安装包，并附加到 draft GitHub Release。推一个 tag 即可发布：

```bash
git tag v0.1.0
git push origin v0.1.0
```

（也可以在该仓库的 **Actions** 标签页手动触发。）目前尚未配置代码签名，所以 macOS/Windows 构建是未签名的——适合早期测试。

## 编辑历史与推送

### 编辑一条提交

1. **打开**一个仓库（Browse… 或粘贴路径），再从下拉框选一个**分支**——git-knife 直接按 ref 编辑它，不做 checkout，因此你的工作区和当前检出保持原样。
2. 点击任意**非 merge** 提交展开编辑器。
3. 修改 message、作者/提交者姓名、邮箱或日期。被编辑的行会高亮；日期字段保留该提交原始的 UTC 偏移。
4. 点击 **Review & apply**，核对 old → new 预览，然后确认。

git-knife 只重写**你的本地分支**。它从不联系远程、从不替你 push——推送永远是显式的一步。

### 批量查找替换

点击提交表格上方的 **Bulk find & replace**，一次性修改多条提交：

- 选择要作用的字段（message、作者/提交者姓名、作者/提交者邮箱——任意组合）。
- 输入 **Find** / **Replace**。开启 **Regex** 可用模式匹配与 `$1` 反向引用，关闭则为字面量搜索。**区分大小写**默认开启。
- 面板会实时统计命中的提交数和替换次数。点击 **Stage edits** 把它们变成高亮行，再照常 **Review & apply**。

示例——把每条提交从旧邮箱迁到新邮箱：目标选 *作者邮箱* + *提交者邮箱*，find `old@example.com`，replace `new@example.com`。merge 提交也会包含，且连续多次操作可以叠加。

### 推送重写后的历史

修改一条提交会改变它的 hash **以及其后所有提交的 hash**，所以你的本地分支与远程已经分叉。普通的 `git push` 会被以 *non-fast-forward* 拒绝。请用带 lease 的强制推送：

```bash
git push --force-with-lease origin <branch>
```

`--force-with-lease` 会在远程自你上次 fetch 后有变动时拒绝推送，因此不会静默覆盖队友的提交。优先于 `--force`，后者跳过了这道安全检查。

> git-knife 在改动触及**已经存在于 upstream 的提交**时，会显示 **"rewrites pushed history"** 警告。如无必要，只编辑未推送的提交——重写共享历史会迫使所有人重新同步。

### 重写共享历史之后

任何已经拉取过旧提交的人，现在都有一份分叉的历史。他们各自把本地分支重新对齐到新的远程状态：

```bash
git fetch origin
git reset --hard origin/<branch>   # 会丢弃仅存在于本地的提交——请先协调
```

### 撤销一次重写

- **应用内：** **Backups** 面板可一键恢复到重写前的 tip。
- **命令行：** 每次 apply 都保存了一个备份 ref——

  ```bash
  git for-each-ref refs/knife-backup      # 找到重写前的 tip
  git reset --hard <backup-ref-or-hash>   # 把分支移回去
  ```

  `git reflog` 也会列出旧 tip。如果已经强制推送过，先在本地恢复，再 `git push --force-with-lease` 一次。

### 签名提交

重写会改变提交的 hash，这会**使它的 GPG/SSH 签名失效**（HN 上被关注过的点）。git-knife 通过原始 `gpgsig` 头检测签名提交——与校验无关，因此即使没有 `allowedSignersFile` 也能识别 SSH 签名——并且：

- 在表格中给它打上 `signed` 标记，
- 在 apply 栏**警告**重写会剥离多少个签名提交，
- 提供 **re-sign** 开关，用你配置的密钥（`user.signingkey` / `gpg.format`）给重建的提交重新签名。如果开启了 re-sign 但没有配置密钥，apply 会在动任何 ref 之前安全失败。

### 透明签名笔记

默认情况下，git-knife 会给每个重写后的 tip 提交附加一条简短、**公开**的笔记，放在独立的 notes ref 上，因此不会碰你的常规笔记：

```bash
git notes --ref=git-knife show <commit>   # 读取它
git for-each-ref refs/notes/git-knife      # 这个仓库被 git-knife 改过吗？
```

它在普通 `git log` 中不可见（独立 ref），但完全可被发现——没有隐藏编码。可在应用内用 **🔪 signature note** 复选框随时关闭（设置会被记住）。要从仓库彻底清除：

```bash
git update-ref -d refs/notes/git-knife
```

## 实现原理

- `src-tauri/src/git.rs` — 唯一派生 `git` 进程的地方。
- `commits.rs` — `open_repo`、`list_commits`（用 NUL/记录分隔符解析）。
- `rewrite.rs` — `preview_edits` + `apply_edits`：从最早被编辑的提交到 tip，用 `commit-tree` 重建整条链，然后保存备份 ref，并用对旧 tip 的 compare-and-swap 移动分支。
- `backup.rs` — 列出 `refs/knife-backup/*` 并用 `git reset --hard` 恢复。

所有 git 子进程都跑在后台线程，绝不占用 UI 线程——打开仓库、切换分支、重写大批历史时，即使底层在频繁派生 git 进程，界面也保持流畅。分支的 ahead/behind 计数来自**单次** `for-each-ref`（不再为每个分支起一个进程）；Windows 下子进程以隐藏控制台的方式启动（`CREATE_NO_WINDOW`），绝不会闪现终端窗口。

## 安全性

每次 apply 都会先创建指向旧 tip 的 `refs/knife-backup/<branch>/<epoch>`，然后才动任何东西。不做任何强制删除；Backups 面板随时可以恢复。

如果重写或恢复失败，错误会同时以应用内 banner 和原生对话框两种方式展示——一次失败的历史重写绝不会被无声漏掉。分支 ref 只在所有重建提交成功后才会移动，所以重写中途失败时分支保持原样。
