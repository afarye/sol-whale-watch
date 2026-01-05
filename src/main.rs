use dotenv::dotenv;
use solana_client::nonblocking::pubsub_client::PubsubClient; // 引入 PubSub 客户端
use solana_client::rpc_config::RpcTransactionLogsConfig;
use solana_client::rpc_config::RpcTransactionLogsFilter;
use solana_client::nonblocking::rpc_client::RpcClient; // nonblocking：这是异步版本，与同步版本solana_client::rpc_client区分
use solana_sdk::commitment_config::CommitmentConfig;
use futures::StreamExt; // 让我们可以用 .next() 遍历数据流
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
    // 加载 .env 文件
    dotenv().ok();
    /*
    dotenv()：函数调用，读取.env文件
    .ok()：将Result<T, E>转换为Option<T>，忽略错误
    如果.env文件不存在也不报错，继续执行
    */
    println!("🚀 启动 Solana 巨鲸监控者 (WebSocket 版)...");

    // 读取环境变量 WS_URL
    let ws_url = env::var("WS_URL").expect("请在 .env 中设置 WS_URL");
    println!("📡 正在连接 WebSocket: {}", ws_url);


    // 创建 PubSub 客户端
    // PubSubClient::new 会返回一个 Result，我们需要解包
    let pubsub_client = PubsubClient::new(&ws_url).await?;
    println!("✅ WebSocket 连接成功!");

    // 定义订阅过滤器
    // 我们监听 "System Program" (11111111111111111111111111111111)
    // 这意味着任何涉及 SOL 转账或系统操作的交易都会被捕获
    let filter = RpcTransactionLogsFilter::Mentions(vec![
        "11111111111111111111111111111111".to_string()
    ]);


        let config = RpcTransactionLogsConfig {
        // processed 级别最快，可能有极低概率回滚，但适合监控
        commitment: Some(CommitmentConfig::processed()), 
    };

    println!("🎧 开始监听 System Program 的日志流...");

    // 订阅日志 (logs_subscribe)
    // 这会返回两个东西：
    // - stream: 一个源源不断吐出数据的流
    // - _unsubscribe: 取消订阅的句柄（这里我们暂不使用，让它一直跑）
    let (mut stream, _unsubscribe) = pubsub_client
        .logs_subscribe(filter, config)
        .await?;

    // 处理数据流 (无限循环)
    // stream.next().await 会在这里“等待”，直到 Solana 推送一条新数据过来
    while let Some(response) = stream.next().await {
        // response.value 包含了日志的具体内容
        let logs = response.value;

        // 打印交易签名 (Signature)
        // 这是每一笔交易的唯一身份证
        // 只有当 logs.err 为 None（表示交易成功），并且 日志数量（logs.logs.len()）大于 5 行时，才打印出来
        if logs.err.is_some() || logs.logs.len() <= 5 {
            continue;
        }

        println!("🔥 捕获新交易: https://solscan.io/tx/{}", logs.signature);
        
        // 打印一点点日志看看 (只打印前3行，防止刷屏)
        for log in logs.logs.iter().take(3) {
            println!("   📝 {}", log);
        }
        println!("---------------------------------------------------");
    }

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


