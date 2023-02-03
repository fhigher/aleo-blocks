### 同步aleo区块数据，并计算区块及solution奖励

#### Checklist
- [x] 同步区块数据
- [x] 计算区块奖励和各个solution奖励
- [x] 根据address过滤条件，数据入库存储
- [ ] 自动更换api
- [ ] 提供查询接口
- [ ] 扩展其它类型数据库

#### 设计思路
1. 使用官方或自己的API，拉取区块json数据, 配置存储指定address相关的数据
2. 同时将height, block_hash, previous_hash, header.metadata, block_reward入库
3. 通过height外键关联记录transactions
4. 通过height外键关联记录solutions
5. 计算block奖励，并计算solution奖励

#### 关于奖励部分区块json字段对应源码结构
    block.coinbase -> coinbase_solution {[partial_solution], proof}, 是所有prove_solution {partial_solution, proof} 的聚合。聚合的proof和每个solution的proof，都是分别用来验证solution

#### 运行方法
    cargo build --release
    设置日志等级(默认info) export RUST_LOG=debug 修改log level
    修改配置文件参数
    ./target/release/aleo-reward


    
    


