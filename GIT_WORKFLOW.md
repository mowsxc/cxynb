# Git 工作流规范

**项目**: meituan-rs  
**版本**: 0.5.2  
**更新**: 2026-07-07

---

## 1. 分支策略

```
main (稳定分支，可部署)
  ↑
dev (开发分支，日常提交)
  ↑
feature/* (功能分支)
  ↑
hotfix/* (紧急修复)
```

### 规则
- `main` 分支永远可编译、可运行
- 新功能在 `feature/xxx` 分支开发
- 完成后合并到 `dev`，测试通过后合并到 `main`
- 紧急修复在 `hotfix/xxx` 分支，直接合并到 `main`

---

## 2. Commit 规范

### 格式
```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Type 列表
| Type | 说明 |
|------|------|
| `feat` | 新功能 |
| `fix` | Bug 修复 |
| `docs` | 文档更新 |
| `style` | 代码格式（不影响功能） |
| `refactor` | 重构 |
| `perf` | 性能优化 |
| `test` | 测试 |
| `chore` | 构建/工具变更 |
| `ci` | CI/CD 变更 |

### Scope 列表
| Scope | 说明 |
|-------|------|
| `backend` | Rust 后端 |
| `frontend` | HTML/JS 前端 |
| `api` | API 接口 |
| `db` | 数据库 |
| `ui` | UI/UX |
| `config` | 配置文件 |
| `docs` | 文档 |

### 示例
```
feat(backend): 纯 Rust 刷新替代 Python 脚本

- 使用 ureq 实现 HTTPS POST 调用美团 API
- 删除 refresh_data.py 依赖
- 添加 Cookie 过期自动检测

BREAKING CHANGE: 不再需要 Python 运行时
```

```
fix(api): 修复 new_count/updated_count 计数反转

- 先检查记录是否存在再计数
- 修复刷新统计数字不准确问题
```

---

## 3. 版本号规范（SemVer）

```
vMAJOR.MINOR.PATCH

MAJOR: 不兼容的 API 变更
MINOR: 向后兼容的功能新增
PATCH: 向后兼容的 Bug 修复
```

### 版本更新时机
- 每次合并到 `main` 时必须更新版本号
- `Cargo.toml` 和 `CHANGELOG.md` 同步更新
- 打 tag: `git tag v0.5.2`

---

## 4. 发布流程

```bash
# 1. 确保 dev 分支稳定
git checkout dev
cargo build --release  # 编译通过
cargo test              # 测试通过

# 2. 更新版本号
# 编辑 Cargo.toml: version = "x.x.x"
# 编辑 CHANGELOG.md: 添加版本记录

# 3. 提交版本更新
git add -A
git commit -m "chore(release): v0.5.2"

# 4. 合并到 main
git checkout main
git merge dev
git tag v0.5.2

# 5. 构建发布
cargo build --release
# target/release/meituan-rs.exe 即为发布产物
```

---

## 5. 每日开发流程

```bash
# 1. 开始新功能
git checkout -b feature/auto-refresh dev

# 2. 开发 + 提交（小步提交）
git add -A
git commit -m "feat(backend): 添加启动时自动刷新"

# 3. 完成后合并回 dev
git checkout dev
git merge feature/auto-refresh
git branch -d feature/auto-refresh

# 4. 测试通过后合并到 main
git checkout main
git merge dev
git tag v0.5.2
```

---

**本文档为 Git 工作流唯一规范，所有开发需遵守。**
