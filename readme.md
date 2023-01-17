设计思路：
1. 使用官方或自己的API，拉取区块json数据, 可配置存储指定address相关的数据，默认是所有区块数据
2. 同时将height, block_hash, previous_hash, previous_state_root, transactions_root, coinbase_accumulator_point, header.metadata, signature, block_reward入库
3. 通过height外键关联记录transactions
4. 通过height外键关联记录coinbase, solution_reward
5. 计算block奖励，并计算solution奖励, 计算当前块，依赖上一个块的数据

关于奖励部分json字段对应源码结构
block.coinbase -> coinbase_solution{ [partial_solution], proof }, 是所有prove_solution {partial_solution, proof} 的聚合
聚合proof和每个solution的proof，都是用来验证partial_solution


设置日志等级： 
export RUST_LOG=debug

TODO：
1. 自动更换api
2. 提供查询接口
3. 扩展其它类型数据库
