use dotenv::dotenv;
use solana_client::nonblocking::rpc_client::RpcClient; 
// nonblocking：这是异步版本，与同步版本solana_client::rpc_client区分
use solana_sdk::commitment_config::CommitmentConfig;
use std::env;

// #[tokio::main] 是一个过程宏，它把 async fn main() 转换成真正启动 Tokio 运行时的代码
/*
展开后的实际代码大致如下：
fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            // 你的async main代码在这里
        })
}
*/
#[tokio::main]
/*
 */
async fn main() -> anyhow::Result<()> {
    // 1. 加载 .env 文件 (虽然现在还没用到 API Key，先养成习惯)
    dotenv().ok();
    /*
    dotenv()：函数调用，读取.env文件
    .ok()：将Result<T, E>转换为Option<T>，忽略错误
    如果.env文件不存在也不报错，继续执行
    */
    println!("🚀 正在启动 Solana 巨鲸监控者...");

    // 2. 定义 RPC 节点地址
    // mainnet-beta 是 Solana 的主网
    // 注意：公共节点有速率限制，生产环境通常用 Helius/QuickNode/Alchemy
    let rpc_url = env::var("RPC_URL").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
    /*
    env::var()：获取环境变量，返回Result<String, env::VarError>
    .unwrap_or_else(|_| ...)：
    如果Result是Ok，提取值
    如果是Err，执行闭包|_| ...
    |_|是闭包参数，_表示忽略错误值
    .to_string()：将字符串字面量&str转换为String（堆分配）
    */
    // 3. 创建异步 RPC 客户端
    // CommitmentConfig::confirmed() 表示我们认为“确认中”的状态就足够了，不用等完全 finalized
    let client = RpcClient::new_with_commitment(rpc_url.to_string(), CommitmentConfig::confirmed());
    /*
    ::new_with_commitment：关联函数（类似Java的静态方法）
    CommitmentConfig::confirmed()：
    confirmed表示交易已被超半数节点确认
    还有processed（刚收到）、finalized（不可逆转）
    */
    println!("📡 正在连接到 Solana 主网: {}", rpc_url);

    // 4. 发起异步请求
    // 这里的 .await 是关键！
    // Java: client.getVersion() 会卡住线程等待网络返回
    // Rust: client.get_version().await 会让出当前线程去干别的事，等网络回包了再回来继续
    let version = client.get_version().await?; 
    /*
    .await：异步等待的关键操作符
    非阻塞：当前async函数会暂停，让出线程控制权，线程可以去执行其他任务
    */
    let block_height = client.get_block_height().await?;

    println!("✅ 连接成功!");
    println!("   Solana 版本: {}", version.solana_core);
    println!("   当前区块高度: {}", block_height);
    
    // 5. 模拟一个简单的并发任务 (可选演示)
    // 只要为了让你感受一下 tokio::spawn
    let handle = tokio::spawn(async {
        println!("   [后台任务] 我是并发执行的小任务，我正在睡觉...");
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        println!("   [后台任务] 我醒了！");
        "任务完成"
    });
    /*
    tokio::spawn：创建新的异步任务
    立即返回JoinHandle<T>，不等待任务完成
    任务会被调度到Tokio运行时执行
    async { ... }：异步块，创建一个匿名异步函数
    tokio::time::sleep：异步睡眠，不阻塞线程
    对比标准库的std::thread::sleep会阻塞整个线程 
    tokio::time::Duration::from_secs(2)：创建一个Duration对象，表示2秒
    */

    // 等待后台任务完成
    let result = handle.await?;
    println!("   [主线程] 后台任务返回: {}", result);
    /*
    handle.await：等待任务完成，返回Result<T, JoinError>
    如果任务正常结束：Ok(T)
    如果任务panic：Err(JoinError)
    */
    Ok(())
}
/*
主线程
   ↓
[tokio::main] 创建运行时
   ↓
运行时.spawn(主Future)
   ↓
主Future.poll()
   ↓
遇到.await → 返回Pending
   ↓
运行时检查其他就绪的任务
   ↓
IO完成 → 唤醒对应任务
   ↓
继续执行.await之后的代码
*/


