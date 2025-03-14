# 状态和存储

*阅读其它语言版本: [English](../state.md), [Korean](state_KR.md), [日本語](state_JP.md).*

## Mwc 的状态

### 结构

一条 Mwc 链的完整状态包含以下所有数据：

1. 完整的未花费输出（UTXO）集。
1. 每个输出的范围证明。
1. 所有交易内核。
1. 针对上述每一项的 MMR（除了输出 MMR 以外包括其它所有输出的哈希，不仅仅是未花费的哈希）。

此外，链中的所有头都必须使用有效的工作证明来锚定上述状态（该状态对应于工作量最大的链）。
我们注意到，一旦验证了每个范围证明并计算了所有内核承诺的总和，就不再严格要求范围证明和内核对节点起作用。

### 验证方式

对于一个完整状态的 Mwc，我们可以验证以下内容：

1. 内核签名针对其承诺（公钥）有效。 这证明内核是有效的。
1. 所有内核承诺的总和等于所有 UTXO 承诺的总和减去总供应量。这证明内核和输出承诺均有效，并且没有创建任何预期之外的代币。
1. 所有 UTXO，范围证明和内核哈希都存在于它们各自的 MMR 中，并且那些 MMR 哈希到有效根。
1. 在给定的时间点上一个已知的工作量最多的块头包括 3 个 MMR 的根。这验证了 MMR，并证明整个状态是由工作量最多的链产生的。

### MMR 与修剪

用于为每个 MMR 中的叶节点生成的哈希数据（除其位置以外，还包括以下内容：

* 输出 MMR 哈希的特征字段和自创世以来所有输出的承诺。
* 范围证明 MMR，对整个范围证明数据进行哈希处理。
* 内核 MMR 哈希了内核的所有字段：功能（feature），费用（fee），锁高度（lock height），超额承诺（excess commitment）和超额签名（excess signature）。

请注意，所有输出，范围证明和内核均以它们在每个块中出现的顺序添加到其各自的 MMR 中（还有一点需要注意，需要对块数据进行排序）。

随着输出被花费掉，其承诺和范围证明数据都可以被删除掉。此外，相应的输出和范围验证 MMR 可以被修剪。

## 状态存储

Mwc 中的输出，范围证明和内核的数据存储很简单：一个 append-only 的文件，通过内存映射来访问数据。
随着输出被花费，删除日志将维护可以删除的职位。这些位置与 MMR 节点位置完全匹配，因为它们均以相同顺序插入。
当删除日志变大时，可以偶尔通过重写相应文件来压缩这些文件，而无需删除它们（同样也是 append-only），并且可以清空删除日志。
对于 MMR，我们需要增加一些复杂性。
