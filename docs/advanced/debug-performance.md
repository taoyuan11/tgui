# Debug 性能优化记录

2026-10-19，Apple M5 / Metal。项目 dev profile 为 `opt-level = 1`，第三方依赖
`opt-level = 3`，保留 debug assertions、溢出检查和 wgpu 默认调试/验证设置。
变更基于 `952adff6`；下列数字为本次同机采样，不能直接当作实际应用 FPS。

## 文本缓存

精确布局与普通测量缓存不再在超限时整表清空，改用有条数和负载字节预算的采样 LRU：
每次在两个伪随机候选中淘汰较旧项。这样不会像严格 LRU 一样，在顺序循环读取
“容量 + 1”项时每次都淘汰下一项。命中仍使用借用键，不分配字符串；精确布局保留共享几何。

- 精确布局：最多 1024 项，文本和几何等负载预算 16 MiB。
- 普通测量：最多 4096 项，负载预算 4 MiB。
- 单项超过预算时不缓存，不挤掉已有工作集。预算不包含哈希表空桶、索引容量及分配器开销。

`text_layout_working_set` 每轮顺序访问所有不同文本，预热后 Criterion 点估计如下。
文本包含中文，测试的是精确布局查询，不含 GPU 绘制。

| 不同文本数 | 修改前 | 修改后 |
| --- | ---: | ---: |
| 256 | 6.939 µs | 7.448 µs |
| 257 | 6.945 µs | 7.503 µs |
| 258 | 7.762 ms | 7.602 µs |
| 512 | 15.131 ms | 15.191 µs |
| 1024 | 30.940 ms | 31.054 µs |
| 1025 | 30.323 ms | 125.380 µs |
| 2048 | 62.154 ms | 62.611 ms |

258 / 512 项的性能断崖已消除，命中路径有小幅访问时间记录开销。两倍容量的循环工作集
仍大量失配，2048 项没有显著改善；有界缓存不能承诺任意工作集都命中。

## 全量布局和场景存储

Taffy 的 measurement context 改为 `Box<MeasureContext>`，不需要测量的叶子不存 context，
避免稀疏 arena 为每个节点预留最大 widget 样式变体。预数节点并一次预留容量；重建索引时
复用 HashMap 存储。删除子树时显式释放 Taffy 单独保存的 context。

场景收集记住上次实际收集的节点数，用作下次分配提示，不再为屏外所有节点预分配场景 map。
无法原地 splice 的简单场景复用根输出数组，减少一次完整根场景克隆分配。
复杂 overlay / portal / focus 等场景仍使用既有回退。祖先重合成仍会复制数组元素。

| 基准 | 修改前 | 修改后 |
| --- | ---: | ---: |
| Dashboard 500 行，全布局 + 收集 | 28.575 ms | 19.044 ms |
| Dashboard 500 行，复用布局重收集 | 0.507 ms | 0.474 ms |

Dashboard 全布局改善约 33%；修改前置信区间为 25.909–31.660 ms，修改后为
18.990–19.098 ms。该基准主要是文本行，有屏外裁剪，不能等同于交互组件树。
另测 `widget_core_layout` 500 行（含 Button），全布局 + 收集为 **25.423 ms**，
复用布局重收集为 **7.720 ms**。全量布局仍为 O(N)，这两种 500 行场景均未达到
16.67 ms 的单帧预算；大量可交互节点仍需虚拟化或进一步缩小布局/收集范围。

## 滚动和顶点准备

修复了两个让 GPU 路径过早回退的问题：隐藏滚动条仍产生 track/thumb 命中几何；普通嵌套
容器覆盖外层滚动 owner。现在只有符合条件的滚动容器建立 owner，内部普通容器继承它。

收集前保守排除嵌套滚动、子容器独立裁剪、弹层、背景模糊和不支持的组件。仅在实际标记的
GPU 子树内保留屏外命令；适配器支持 IMMEDIATES 本身不再关闭整棵树的 CPU 裁剪。
运行时拒绝没有标记命令的场景，防止把已裁掉屏外内容的 CPU 场景错误当成 GPU 场景。
后代滚动区域保存未裁剪 frame，平移后重新求交集，支持从屏外滚回屏内。

纯 GPU 滚动允许复用完整 prepared-frame 缓存：保留对应轮转槽内的顶点和绑定，只更新
per-draw 平移量，跳过 staging 复制和顶点字节 diff。仍验证 scene / resource / content /
vertex generation、viewport 和字体身份；变换记录或不支持的 effects 继续回退。

| 600 行运行时基准 | 修改前 | 修改后 |
| --- | ---: | ---: |
| 默认子容器裁剪，CPU patch，含 GPU 等待 | 3.859 ms | 3.615 ms |
| 默认子容器裁剪，强制全量重收集，含 GPU 等待 | 13.950 ms | 3.448 ms |

最终默认裁剪场景的阶段均值：scene 1.170 ms、prepare + upload 0.387 ms。
普通 Flex 默认 `Overflow::Hidden`，因此这组场景仍正确回退到 CPU；交互节点也继续保留
必要的命中、生命周期等 metadata，不作不安全的屏外跳过。

另设子容器 `Overflow::Visible` 的 GPU 路径夹具：scene **0.112 ms**、prepare + upload
**0.010 ms**，含 GPU 等待总计 **3.359 ms**；120 帧探针命中 prepared 缓存 118 次。
这是不同裁剪条件的场景，不能把其阶段收益直接当成默认场景或实际 FPS 的提升。
在滚动到 180 / 2000 / 0 的位置，保留式 GPU 输出与全量重收集的像素逐字节一致。

## 模糊、阴影和调试校验

- 背景模糊按半径缓存最多 8 组不可变 uniform / bind group；离屏目标重建时失效。
  不同半径不会通过同一 uniform 的覆盖写互相影响。同一 GPU 基准中，缓存绑定约
  2.567 ms，强制清空绑定缓存约 2.661 ms，置信区间有重叠，不据此宣称稳定加速比例。
  两者像素完全一致。全目标 snapshot、两遍模糊和合成仍然存在。
- 阴影 miss 在缓存锁外栅格化，安装结果时再次查重；修正命中时的 recency，避免常用项
  被错误淘汰。miss 本身仍同步执行 CPU 栅格化/模糊，没有引入异步占位行为。
- atlas 整页释放校验合并重复计数表并使用 hashbrown，保留全部校验。
  批量释放按页和坐标排序，避免 HashMap 随机释放顺序引起矩形合并成本波动。
  本机混合页 liveness 探针从约 4.8 ms 降到 0.543 ms；逐项释放对照为 0.548 ms。
  校验像素、存活 allocation 和页面数量的断言，以及原有 5% 性能门槛均保留。

## 复现

串行运行，避免同时编译、测试或开启 GUI 示例影响计时。文本表使用 0.2 秒预热、
0.5 秒测量、10 个样本；最终布局/GPU 表使用以下参数。Criterion 的默认 `change`
对比上次运行，若要比较修改前版本，应在相同 fixture 下另存 baseline。

```sh
cargo bench -p tgui-benchmarks --bench text_processing --profile dev --features bench-support -- \
  text_layout_working_set --warm-up-time 0.2 --measurement-time 0.5 --sample-size 10

cargo bench -p tgui-benchmarks --bench real_widget_pipeline --profile dev --features bench-support -- \
  'real_widget_(full_layout_and_scene|scene_recollect_only)' \
  --warm-up-time 0.5 --measurement-time 1 --sample-size 20

cargo bench -p tgui-benchmarks --bench widget_core_layout --profile dev --features bench-support -- \
  'widget_full_layout_and_scene/500|widget_scene_recollect_cached_layout/500' \
  --warm-up-time 0.5 --measurement-time 1 --sample-size 20

cargo bench -p tgui-benchmarks --bench runtime_gpu_pipeline --profile dev --features bench-support -- \
  'tooltip_effect_gpu|runtime_scroll_renderer_gpu_submit|runtime_gpu_scroll_retained_vertices' \
  --warm-up-time 0.5 --measurement-time 1 --sample-size 20
```

## 正确性覆盖

新增测试覆盖缓存循环超限、字节预算、超大条目、512 项 Unicode 换行布局、4097 项普通测量，
以及 GPU 滚动与全量重收集的命令/命中/滚动区域等价、可见滚动条/嵌套滚动/无标记场景回退、
prepared 平移更新和阴影锁/淘汰行为。GPU 基准另验滚动与模糊的精确像素等价。

本次验证结果：1511 项 runtime 单元测试通过；默认 / no-default / audio / video
和 workspace 全目标检查通过。保留仓库已有的 renderer tests 未使用 import 警告。
另外修正了分页器 benchmark 的旧夹具，显式传入四个 page-size 选项以匹配既有断言，
避免默认值已为空时测到两组相同场景。

验证命令：

```sh
cargo test -p tgui-runtime --lib -- --test-threads=1
cargo check -p tgui
cargo check -p tgui --no-default-features
cargo check -p tgui --features audio
cargo check -p tgui --features video
cargo check --workspace --all-targets
cargo fmt --all -- --check
git diff --check
```
