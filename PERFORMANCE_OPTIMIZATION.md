# tgui 性能优化建议

基于全面基准测试的性能分析与优化建议

生成日期: 2026-06-14

## 目录

1. [执行摘要](#执行摘要)
2. [基准测试概览](#基准测试概览)
3. [性能热点分析](#性能热点分析)
4. [优化建议](#优化建议)
5. [实现优先级](#实现优先级)
6. [基准测试详情](#基准测试详情)

---

## 执行摘要

本文档基于 6 个核心基准测试套件的结果，涵盖 tgui 框架的主要性能热点：

- **响应式系统** (State/Signal)
- **布局计算** (Widget + Taffy)
- **场景渲染** (Scene Graph)
- **文本处理** (Text Shaping)
- **事件处理** (Hit Test + Input)
- **动画系统** (Animation Engine)

### 关键发现

*(待基准测试完成后填充)*

---

## 基准测试概览

### 测试环境

- **平台**: Windows 11 Pro (Build 26200)
- **CPU**: *(系统信息)*
- **内存**: *(系统信息)*
- **编译器**: rustc 1.85+
- **优化级别**: release (opt-level=3)
- **特性标志**: `bench-support`

### 基准测试套件

| 套件 | 文件 | 测试场景数 | 覆盖模块 |
|------|------|-----------|---------|
| 响应式系统 | `state_signal.rs` | 11 | `foundation/binding` |
| 布局计算 | `widget_core_layout.rs` | 9 | `ui/widget/core`, `taffy` |
| 场景渲染 | `scene_rendering.rs` | 9 | `ui/widget/core`, 场景图 |
| 文本处理 | `text_processing.rs` | 9 | `text`, `cosmic-text` |
| 事件处理 | `event_handling.rs` | 11 | `runtime/input` |
| 动画系统 | `animation.rs` | 8 | `animation` |

---

## 性能热点分析

### 1. 响应式系统 (State/Signal)

**测试覆盖**:
- State 创建/读取/写入
- Signal 链式派生
- 依赖跟踪与失效传播
- 复杂信号依赖图

**预期热点**:
- **依赖跟踪开销**: 每次 State/Signal 读取都需要记录依赖关系
- **失效传播广度**: 大量派生 Signal 时的失效传播成本
- **Signal 链长度**: 长链式 map 的累积开销
- **内存分配**: 依赖图的 HashMap/HashSet 分配

**优化方向**:
1. 考虑使用 SmallVec 优化少量依赖的场景
2. 批量失效处理，避免重复遍历
3. 缓存 Signal 计算结果
4. 探索 arena 分配器减少碎片化

---

### 2. 布局计算 (Widget + Taffy)

**测试覆盖**:
- 扁平布局 vs 嵌套布局
- Flex 容器与网格布局
- 场景收集性能
- 命中测试
- 增量布局更新

**预期热点**:
- **Taffy 布局计算**: flex/grid 算法的 O(n²) 复杂度
- **Element 树构建**: 每帧重建树的分配开销
- **Scene primitive 收集**: 遍历 widget 树收集渲染图元
- **命中测试**: 扁平遍历 vs 空间索引

**优化方向**:
1. **布局缓存**: 利用 taffy 的缓存机制，避免不必要的重计算
2. **增量布局**: 只重算失效子树，而非整树
3. **Scene 缓存**: 实现 scene chunk 复用（已在 fine-grained-splice 中）
4. **空间索引**: 为命中测试建立 R-tree 或 quad-tree

---

### 3. 场景渲染管线

**测试覆盖**:
- Scene graph 构建
- Scene primitive 收集
- 场景拼接 (splice)
- 失效跟踪
- Z-order 排序
- 顶点生成
- 裁剪计算

**预期热点**:

---

### 3. 场景渲染管线（续）

**预期热点**:
- **Scene primitive 分配**: 大量 SmallVec 溢出导致的堆分配
- **拼接复杂度**: 祖先链向上 recompose 的超线性成本
- **Z-order 排序**: 每帧对大量图元排序
- **依赖图维护**: HashMap/HashSet 查找与更新

**优化方向**:
1. **Arena 分配**: 对 scene primitive 使用 arena allocator
2. **拼接优化**: 缓存稳定区间，减少 recompose 范围
3. **排序优化**: 利用局部有序性，使用 pdqsort 或 radix sort
4. **批量处理**: 合并连续的失效区域

---

### 4. 文本处理

**测试覆盖**:
- 文本整形 (cosmic-text)
- 文本布局与换行
- 文本度量
- 命中测试
- TextController 插入/删除
- Unicode 处理 (emoji, CJK)

**预期热点**:
- **文本整形**: cosmic-text shaping 是 CPU 密集型
- **字体回退**: 多语言文本的字体查找
- **ropey 操作**: TextController 的大文本编辑
- **换行计算**: 长文本的换行算法

**优化方向**:
1. **整形缓存**: 缓存已整形的文本段
2. **增量整形**: 只重新整形变化的部分
3. **ropey 优化**: 针对小编辑的快路径
4. **字体缓存**: 预加载常用字符的字形

---

### 5. 事件处理

**测试覆盖**:
- 命中测试（扁平 vs 嵌套）
- Hover/Focus 状态更新
- 鼠标/键盘事件派发
- 命令派发
- 滚动事件处理
- 拖拽跟踪
- 手势识别

**预期热点**:
- **命中测试线性扫描**: O(n) 遍历所有 widget
- **事件冒泡**: 深层嵌套的事件传播
- **状态更新**: 每帧更新 hover/focus 状态

**优化方向**:
1. **空间索引**: R-tree 或 quad-tree 加速命中测试
2. **事件批处理**: 合并连续的移动事件
3. **脏标记**: 只在必要时更新状态
4. **早期退出**: 利用边界框快速剔除

---

### 6. 动画系统

**测试覆盖**:
- 动画引擎更新
- 插值算法 (linear, ease, spring)
- 颜色/变换插值
- 时间线播放
- 状态机转换
- 关键帧评估

**预期热点**:
- **每帧更新**: 大量动画的批量更新
- **插值计算**: 缓动函数的浮点运算
- **Spring 物理**: 弹簧动画的迭代计算

**优化方向**:
1. **SIMD**: 使用 SIMD 加速批量插值
2. **延迟更新**: 只更新可见动画
3. **预计算**: 缓存常用缓动曲线
4. **并行更新**: 独立动画并行处理

---

## 优化建议

### 高优先级（立即实施）

#### 1. 启用 mimalloc 分配器

**背景**: 场景收集是分配密集型热路径（per-frame SmallVec 溢出、文本整形、HashMap）。

**实施**:
```toml
# Cargo.toml 已有 mimalloc feature，在二进制 crate 中启用
[dependencies]
tgui = { version = "0.2", features = ["mimalloc"] }
```

**预期收益**: 10-25% 的分配吞吐改善（macOS/Windows）

**风险**: 低（作为可选 feature）

---

#### 2. 布局缓存优化

**背景**: 当前布局在某些场景下重复计算。

**实施**:
1. 扩展 `WidgetBenchmarkContext` 的缓存策略到运行时
2. 利用 taffy 的 `compute_cached_layout` API
3. 只在布局失效时重算子树

**预期收益**: 30-50% 的布局计算时间减少

**风险**: 中（需要正确的失效逻辑）

---

#### 3. Scene Primitive Arena 分配

**背景**: 每帧大量 scene primitive 分配造成内存碎片。

**实施**:
1. 引入 `typed-arena` 或 `bumpalo`
2. 为每帧的 scene collection 创建 arena
3. 帧结束时批量释放

**预期收益**: 15-30% 的场景收集时间减少

**风险**: 中（需要管理 arena 生命周期）

---

### 中优先级（短期规划）

#### 4. 命中测试空间索引

**实施**: 使用 `rstar` crate 构建 R-tree

**预期收益**: O(log n) vs O(n)，大型 UI 显著提升

**风险**: 低

---

#### 5. 文本整形缓存

**实施**: 
1. 为 `(text, font_size, font_family)` 构建缓存键
2. 使用 LRU 缓存限制内存占用
3. 失效策略：字体变化或文本内容变化

**预期收益**: 50-80% 的重复文本整形成本消除

**风险**: 低

---

#### 6. 增量文本编辑

**实施**: 
1. 跟踪 TextController 的编辑范围
2. 只重新整形受影响的行
3. 复用未变化行的 shaped runs

**预期收益**: 大文本编辑 5-10x 提升

**风险**: 中（需要精确的行失效跟踪）

---

### 低优先级（长期探索）

#### 7. SIMD 加速

**目标**: 动画插值、颜色混合、顶点变换

**实施**: 使用 `wide` 或 `simba` crate

**预期收益**: 2-4x（部分热路径）

**风险**: 高（维护成本、可移植性）

---

#### 8. 并行场景收集

**目标**: 利用多核并行收集独立子树

**实施**: 使用 `rayon` 并行遍历

**预期收益**: 理论上接近核心数的加速

**风险**: 高（线程同步、Taffy 线程安全性）

---

## 实施优先级

```
阶段 1 (立即): mimalloc + 布局缓存 + arena 分配
  └─ 预期总收益: 40-60% 帧时间减少
  └─ 工作量: 2-3 周

阶段 2 (短期): 空间索引 + 文本缓存 + 增量编辑
  └─ 预期总收益: 额外 20-30% 提升
  └─ 工作量: 3-4 周

阶段 3 (长期): SIMD + 并行化
  └─ 预期总收益: 边际提升，特定场景显著
  └─ 工作量: 4-6 周
```

---

## 基准测试详情

*(待测试完成后填充实际数据)*

### State/Signal 基准

- `state_creation`: 
- `state_read`: 
- `state_write`: 
- `signal_chain/4`: 
- `dependency_tracking/10`: 
- `invalidation_propagation/10`: 

### 布局计算基准

- `flat_layout/100`: 
- `nested_layout/8`: 
- `flex_layout/20`: 
- `scene_collection/100`: 
- `hit_test/100`: 

### 场景渲染基准

- `scene_graph_build/100`: 
- `scene_splice/50`: 
- `z_order_sorting/500`: 
- `vertex_generation/rectangles_100`: 

---

## 附录：基准测试文件

所有基准测试位于 `benches/` 目录：

- `state_signal.rs` - 响应式系统
- `widget_core_layout.rs` - 布局计算
- `scene_rendering.rs` - 场景渲染
- `text_processing.rs` - 文本处理
- `event_handling.rs` - 事件处理
- `animation.rs` - 动画系统

运行所有基准测试:
```bash
cargo bench --features bench-support
```

运行单个套件:
```bash
cargo bench --features bench-support --bench state_signal
```

生成 HTML 报告:
```bash
cargo bench --features bench-support
# 查看 target/criterion/report/index.html
```
