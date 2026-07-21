# 美团订单管理系统 - 设计系统规范 v1.0

**项目**: meituan-rs  
**版本**: 0.5.1  
**更新**: 2026-07-05  
**状态**: 唯一权威参考

---

## 1. 设计原则

1. **一致性优先** - 所有组件使用相同的颜色、间距、圆角
2. **语义化颜色** - 颜色有明确含义，不随意使用
3. **可访问性** - 对比度 ≥ 4.5:1，焦点可见
4. **性能友好** - 动画 ≤ 300ms，避免复杂 transform
5. **响应式** - 手机端优先简化，PC端完整功能

---

## 2. 字体系统

### 2.1 字体栈
```css
--font-sans: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", 
             "PingFang SC", "Microsoft YaHei", "Helvetica Neue", sans-serif;
--font-mono: "SF Mono", Consolas, "Liberation Mono", Menlo, monospace;
```

### 2.2 字号层级
| 变量 | 值 | 用途 | 行高 |
|------|-----|------|------|
| `--text-xs` | 10px | 徽章、标签 | 1.4 |
| `--text-sm` | 11px | 辅助、图例 | 1.5 |
| `--text-base` | 13px | 正文、表格 | 1.5 |
| `--text-md` | 14px | 按钮、输入框 | 1.5 |
| `--text-lg` | 16px | 标题、弹窗 | 1.5 |
| `--text-xl` | 20px | 数字、大标题 | 1.3 |

### 2.3 字重
| 变量 | 值 | 用途 |
|------|-----|------|
| `--font-normal` | 400 | 正文 |
| `--font-medium` | 500 | 按钮、标签 |
| `--font-semibold` | 600 | 标题、强调 |
| `--font-bold` | 700 | 数字、重要信息 |

---

## 3. 颜色系统

### 3.1 主色板
```css
--primary: #ff6600;
--primary-hover: #e55a00;
--primary-active: #cc5000;
--primary-light: #fff7ed;
```

### 3.2 语义颜色
| 变量 | 十六进制 | 含义 | 对比度 |
|------|----------|------|--------|
| `--success` | `#10b981` | 成功、计费价、折扣 | 4.8:1 |
| `--danger` | `#ef4444` | 错误、退款、删除 | 5.1:1 |
| `--cola` | `#7c3aed` | 可乐订单高亮 | 5.3:1 |
| `--financial` | `#c2410c` | 财务价 | 5.0:1 |
| `--text` | `#1f2937` | 正文 | - |
| `--text-muted` | `#6b7280` | 辅助文字 | 7.2:1 |
| `--text-subtle` | `#9ca3af` | 占位符 | 4.7:1 |

### 3.3 背景与边框
```css
--bg: #f0f2f5;
--card: #ffffff;
--hover: #f9fafb;
--border: #e5e7eb;
--border-strong: #d1d5db;
```

### 3.4 状态颜色
| 状态 | 背景 | 文字 |
|------|------|------|
| 退款 | `#fff5f5` | `#991b1b` |
| 可乐 | `#faf5ff` | `#581c87` |
| 完成 | `#ecfdf5` | `#065f46` |

---

## 4. 间距系统（8px 网格）

```css
--space-0: 0;
--space-1: 4px;
--space-2: 8px;
--space-3: 12px;
--space-4: 16px;
--space-5: 20px;
--space-6: 24px;
--space-8: 32px;
```

**使用规则**：
- 面板内边距：`--space-4`
- 元素间距：`--space-2` 或 `--space-3`
- 表格单元格：`6px 10px`（特殊）
- 按钮内边距：`6px 14px`

---

## 5. 圆角系统

```css
--radius-sm: 4px;    /* 徽章 */
--radius-md: 6px;    /* 按钮、输入框、表格 */
--radius-lg: 10px;   /* 卡片、面板 */
--radius-xl: 14px;   /* 弹窗 */
--radius-full: 999px; /* 圆形徽章 */
```

---

## 6. 阴影系统

```css
--shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.05);
--shadow-md: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
--shadow-lg: 0 10px 15px -3px rgba(0, 0, 0, 0.1);
```

---

## 7. 过渡动画

```css
--transition-fast: 80ms cubic-bezier(0.4, 0, 0.2, 1);
--transition-base: 150ms cubic-bezier(0.4, 0, 0.2, 1);
--transition-slow: 300ms cubic-bezier(0.4, 0, 0.2, 1);
```

**使用原则**：
- 颜色/背景：`--transition-fast`
- 变换/尺寸：`--transition-base`
- 复杂动画：`--transition-slow`

---

## 8. 组件规范

### 8.1 按钮

```css
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 6px 14px;
  font-size: 13px;
  font-weight: 500;
  border-radius: 6px;
  border: 1px solid;
  cursor: pointer;
  transition: all var(--transition-base);
  user-select: none;
  line-height: 1.5;
}

/* Primary */
.btn-primary {
  background: var(--primary);
  color: white;
  border-color: var(--primary);
}
.btn-primary:hover { background: var(--primary-hover); }
.btn-primary:active { 
  background: var(--primary-active);
  transform: scale(0.98);
}
.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* Gray */
.btn-gray {
  background: #f9fafb;
  color: var(--text);
  border-color: var(--border);
}
.btn-gray:hover { background: #f3f4f6; }
.btn-gray:active { background: #e5e7eb; }

/* Green */
.btn-green {
  background: var(--success);
  color: white;
  border-color: var(--success);
}
.btn-green:hover { background: var(--success-hover); }

/* Small */
.btn-sm {
  padding: 4px 10px;
  font-size: 12px;
}
```

### 8.2 标签/Tag

```css
.tag {
  display: inline-flex;
  align-items: center;
  padding: 5px 14px;
  font-size: 12px;
  font-weight: 500;
  border: 1px solid var(--border);
  background: white;
  color: var(--text-muted);
  border-radius: 999px;
  cursor: pointer;
  transition: all var(--transition-base);
  user-select: none;
}

.tag.on {
  background: var(--primary);
  color: white;
  border-color: var(--primary);
}

.tag.on-red {
  background: var(--danger);
  color: white;
  border-color: var(--danger);
}

.tag.on-green {
  background: var(--success);
  color: white;
  border-color: var(--success);
}
```

### 8.3 徽章/Badge

```css
.badge {
  display: inline-flex;
  align-items: center;
  padding: 1px 7px;
  font-size: 10px;
  font-weight: 600;
  border-radius: 10px;
  line-height: 1.4;
}

/* 红色：退款 */
.badge-red {
  background: #fee2e2;
  color: #991b1b;
}

/* 紫色：可乐 */
.cola-badge {
  display: inline-block;
  background: #7c3aed;
  color: white;
  font-size: 9px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 10px;
  margin-right: 4px;
  vertical-align: middle;
}
```

### 8.4 输入框

```css
input[type="text"],
input[type="number"],
input[type="time"],
input[type="date"],
select {
  padding: 6px 10px;
  font-size: 13px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: white;
  color: var(--text);
  transition: border-color var(--transition-fast),
              box-shadow var(--transition-fast);
  line-height: 1.5;
}

input:focus,
select:focus {
  outline: none;
  border-color: var(--primary);
  box-shadow: 0 0 0 3px rgba(255, 102, 0, 0.1);
}

input::placeholder {
  color: var(--text-subtle);
}

input:disabled {
  background: #f9fafb;
  color: var(--text-subtle);
  cursor: not-allowed;
}
```

### 8.5 表格

```css
table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
  background: white;
}

thead th {
  position: sticky;
  top: 0;
  background: #f9fafb;
  padding: 8px 10px;
  text-align: left;
  font-weight: 600;
  color: var(--text-muted);
  border-bottom: 2px solid var(--border);
  white-space: nowrap;
  z-index: 1;
}

thead th.sortable {
  cursor: pointer;
  user-select: none;
}
thead th.sortable:hover {
  color: var(--primary);
}

td {
  padding: 6px 10px;
  border-bottom: 1px solid #f3f4f6;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

tbody tr:hover td {
  background: #f9fafb;
}

tbody tr.refunded td {
  background: #fff5f5;
  color: #991b1b;
}
tbody tr.refunded:hover td {
  background: #fee2e2;
}

tbody tr.cola td {
  background: #faf5ff;
}
tbody tr.cola:hover td {
  background: #f3e8ff;
}
```

### 8.6 卡片/面板

```css
.panel,
.bar {
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: var(--shadow-sm);
}
```

### 8.7 弹窗

```css
.modal-box {
  background: white;
  border-radius: 14px;
  padding: 22px;
  max-width: 96%;
  width: 520px;
  max-height: 88vh;
  overflow-y: auto;
  box-shadow: var(--shadow-lg);
}

.modal-box .x {
  position: absolute;
  top: 12px;
  right: 16px;
  cursor: pointer;
  font-size: 24px;
  color: #bbb;
  transition: color var(--transition-fast);
}
.modal-box .x:hover {
  color: #333;
}
```

---

## 9. 交互反馈

| 组件 | Hover | Active | Focus | Disabled |
|------|-------|--------|-------|----------|
| 按钮 | 亮度+5% | 缩放0.98 | 橙色光环 | 透明度0.5 |
| 标签 | 背景变灰 | 背景变深 | - | 透明度0.5 |
| 表格行 | 背景变灰 | - | - | - |
| 输入框 | - | - | 橙色光环 | 灰色背景 |
| 徽章 | 缩放1.1 | - | - | - |

---

## 10. 动画时长

| 用途 | 时长 |
|------|------|
| 颜色/背景 | 80ms |
| 变换/尺寸 | 150ms |
| 柱状图高度 | 300ms |
| 弹窗淡入 | 200ms |

---

## 11. 响应式

| 断点 | 策略 |
|------|------|
| > 768px | 完整功能 |
| ≤ 768px | 隐藏非关键列、紧凑间距、横向滚动 |

---

**本文档为唯一设计参考，任何改动需更新此文档。**
