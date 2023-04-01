## Sync aleo blocks data, calculate block and solution reward, and support api query.

[![License](https://img.shields.io/badge/license-MIT-blue)](https://raw.githubusercontent.com/ipdr/ipdr/master/LICENSE)
[![stability-stable](https://img.shields.io/badge/stability-stable-green.svg)](https://github.com/emersion/stability-badges#stable)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](#contributing)

### checklist
- [x] sync block and solution data
- [x] auto alternative api
- [x] according to the address filter conditions, specific data is stored in the database
- [x] calculate block and solution reward
- [x] query solution and reward
- [ ] query block and reward

### solution proof and aggregation proof
    block.coinbase -> coinbase_solution {[partial_solution], proof}

    aggregate all: prove_solution {partial_solution, proof}

    aggregation proof and the proof of each solution are seperately used to validate solution

### how to 
    1. import the file "aleo-blocks.sql" in mysql database.

    2. cd aleo-blocks & cargo build --release.

    3. export RUST_LOG=debug, set log level, default is info.

    4. modify config file

    5. run
        a. sync blocks service:     
            ./target/release/aleo-blocks sync start 

        b. api service:             
            ./target/release/aleo-blocks api start

        c. view or update the sync height record file：
            ./target/release/aleo-blocks sync check/update
    


    
    


