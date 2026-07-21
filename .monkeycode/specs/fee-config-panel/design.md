# 计费价格配置面板

Feature Name: fee-config-panel
Updated: 2026-07-05

## Description
互联网网吧管理系统的美团订单计费规则配置弹窗。管理员通过配置"关键词1"（套餐类型）+"关键词2"（时长/条件）双关键词匹配订单 `product_info`，系统返回对应的计费价。

## Design System（沿用项目已有变量）

```
--primary:  #ff6600  主色/强调
--success:  #10b981  计费价（绿色）
--danger:   #ef4444  删除（红色）
--text:     #1f2937  正文
--muted:    #6b7280  副标题/标签
--border:   #e5e7eb  分割线
--bg:       #f0f2f5  页面背景
--card:     #fff     卡片背景
```

## Architecture

```
┌──────────────────────────────────────────────────┐
│ ⚙ 计费价格配置              关键词匹配→计费价     │
├──────────────────────────────────────────────────┤
│ ┌─────────────────────────┬──────────┬─────────┐ │
│ │ 套餐类型      │ 时长/条件      │ 计费价│   操作 │ │ ← 表头 大写+字间距
│ ├─────────────────────────┼──────────┼─────────┤ │
│ │ 词1输入框     │ 词2输入框      │ ¥数字 │  ✕    │ │ ← 行hover才显删除
│ │ 新会员       │ 特惠          │ ¥30   │  ✕    │ │
│ │ 5070显卡     │ 3小时体验     │ ¥34   │  ✕    │ │ ← 计费价绿色等宽
│ │ ...          │ ...           │ ...   │  ...  │ │
│ └─────────────────────────┴──────────┴─────────┘ │
├──────────────────────────────────────────────────┤
│                           [+ 添加] [保存]        │
└──────────────────────────────────────────────────┘
```

## Components

### 弹窗容器（modal-box）
- 宽度 `auto` (随表格内容收缩)，最大 `max-width: 600px`
- 圆角 `border-radius: 12px`，阴影 `box-shadow: 0 10px 40px rgba(0,0,0,.15)`
- 沿用 `.modal-box` 样式

### 表格容器（.fee-card）
- `border: 1px solid var(--border)`
- `border-radius: 10px`（与 .panel/.bar 一致）
- `overflow: hidden`
- `max-height: 60vh` 纵向滚动
- 吸顶表头 `thead th { position: sticky; top: 0; z-index: 2 }`

### 表格（.fee-grid）
- `table-layout: auto`（列宽按实际内容分配）
- `border-collapse: separate; border-spacing: 0`
- 列分隔线：`td + td { border-left: 1px solid #f3f4f6 }`
- 计费价列：`width: 60px`（唯一固定宽度）
- 操作列：`width: 36px`
- 关键词列：无显式宽度，浏览器按最大内容自适应

### 表头（thead th）
- `text-transform: uppercase`
- `letter-spacing: 0.05em`（与 .monthly-title 一致）
- `font-size: 11px; font-weight: 600`
- `color: var(--muted)`
- `background: var(--card)`
- `border-bottom: 2px solid var(--border)`
- `padding: 10px 14px`

### 输入框（input）
- `border-radius: 6px`（与 .filters input 一致）
- `border: 1px solid var(--border)`
- `padding: 7px 10px`
- `width: 100%; box-sizing: border-box`
- `font-size: 13px; color: var(--text)`
- `transition: border-color .15s, box-shadow .15s`
- 聚焦：`border-color: var(--primary); box-shadow: 0 0 0 2px rgba(255,102,0,.12)`
- Placeholder：颜色 `color: #c0c4cc; font-style: italic`

### 计费价列（td.fee-val）
- `text-align: right`
- Input：`font-family: "SF Mono", Consolas, monospace; color: var(--success); font-weight: 600; font-size: 13px`
- 添加前缀 `¥` via CSS `content`

### 操作列（td.op-cell）
- `width: 36px; text-align: center`
- Delete button：
  - `width: 26px; height: 26px`
  - `border-radius: 6px`
  - `border: none; background: transparent; cursor: pointer`
  - `color: #ccc → var(--danger) on hover`
  - `opacity: 0 → 1 on row hover`
  - `transition: opacity .15s, color .15s`

### 行状态（tr）
- `border-bottom: 1px solid #f3f4f6`
- Hover：`background: #fafbfc`，同时删除按钮 `opacity: 1`
- 隐藏行（逻辑删除）：`display: none`

### 底栏操作
- 按钮沿用 `.btn` 设计系统
- `添加` = `btn btn-gray`
- `保存` = `btn btn-primary`
- `display: flex; gap: 8px; justify-content: flex-end; margin-top: 16px`
