### 同步aleo区块数据，计算区块及solution奖励，钱包，转账，交易查询处理等相关工具

#### Checklist
- [x] 同步区块及solution数据
- [x] 计算区块奖励和各个solution奖励
- [x] 根据address过滤条件，特定数据入库存储
- [ ] 同步交易数据
- [ ] 自动更换api
- [x] 查询solution及其奖励接口
- [ ] 生成aleo钱包及转账
- [ ] 交易查询处理 

#### 设计思路
1. 使用官方或自己的API，拉取区块json数据, 配置存储指定address相关的数据
2. 同时将height, block_hash, previous_hash, header.metadata, block_reward入库
3. 通过height外键关联记录transactions
4. 通过height外键关联记录solutions
5. 计算block奖励，并计算solution奖励

#### 关于奖励部分区块json字段对应源码结构
    block.coinbase -> coinbase_solution {[partial_solution], proof}, 是所有prove_solution {partial_solution, proof} 的聚合。聚合的proof和每个solution的proof，都是分别用来验证solution

#### 使用方式
    1. 将aleo-blocks.sql导入mysql数据库
    2. 编译 cargo build --release
    3. 设置日志等级(默认info) export RUST_LOG=debug 修改log level
    4. 修改配置文件，调整参数
    5. 启动所需服务或命令，如下：
        a. 同步服务 ./target/release/aleo-tools sync start 
        b. api服务 ./target/release/aleo-tools api start
        c. 查看或更新已同步高度的文件记录 ./target/release/aleo-tools sync check/update
    


    
    


