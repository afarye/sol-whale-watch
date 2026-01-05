use dotenv::dotenv;
use solana_client::nonblocking::pubsub_client::PubsubClient;
use solana_client::rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter};
use solana_client::nonblocking::rpc_client::RpcClient;
use tokio::sync::mpsc;
use solana_sdk::commitment_config::CommitmentConfig;
use futures::StreamExt;
use solana_transaction_status::UiTransactionEncoding;
use solana_sdk::signature::Signature; // 需要用来解析签名字符串
use std::env;
use std::str::FromStr; // 需要用来把 String 转 Signature
use std::sync::Arc; // <--- 引入 Arc 实现共享

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    println!("🚀 启动 Solana 巨鲸监控者 (并发架构版)...");

    let ws_url = env::var("WS_URL").expect("WS_URL 未设置");
    let rpc_url = env::var("RPC_URL").expect("RPC_URL 未设置");

    // 1. 创建管道
    let (tx, mut rx) = mpsc::channel::<String>(100);

    // 2. 启动后台消费者 (调度中心)
    tokio::spawn(async move {
        println!("👨‍🔧 后台调度中心已就位...");
        
        // 创建 RPC 客户端并用 Arc 包裹
        let rpc_client = RpcClient::new(rpc_url);
        let client_arc = Arc::new(rpc_client);

        while let Some(signature) = rx.recv().await {
            // 克隆 Arc 指针 (成本极低)
            let client_ref = client_arc.clone();
            
            // 🔥 关键：为每一笔交易开启一个独立的轻量级线程
            // 这样前一笔交易卡住不会影响下一笔
            tokio::spawn(async move {
                if let Err(e) = process_transaction(client_ref, signature).await {
                    // 打印错误以便调试 (如果是 'not found' 可以忽略，但现在先看看)
                    // eprintln!("❌ 处理失败: {}", e);
                }
            });
        }
    });

    // 3. 生产者：WebSocket 监听
    println!("📡 连接 WebSocket...");
    let pubsub_client = PubsubClient::new(&ws_url).await?;
    // 监听 System Program (SOL 转账)
    let filter = RpcTransactionLogsFilter::Mentions(vec!["11111111111111111111111111111111".to_string()]);
    let config = RpcTransactionLogsConfig {
        commitment: Some(CommitmentConfig::processed()),
    };
    let (mut stream, _unsub) = pubsub_client.logs_subscribe(filter, config).await?;

    println!("🎧 监听中... (阈值: > 0.1 SOL)");

    while let Some(response) = stream.next().await {
        let logs = response.value;

        // 🛠️ 修复 1：不要过滤 logs.len() <= 5
        // 只过滤掉失败的交易 (err.is_some())
        if logs.err.is_some() {
            continue;
        }

        if let Err(_) = tx.send(logs.signature.clone()).await {
            println!("后台已关闭");
            break;
        }
    }

    Ok(())
}

// 接收 Arc<RpcClient>
async fn process_transaction(client: Arc<RpcClient>, signature_str: String) -> anyhow::Result<()> {
    let signature = Signature::from_str(&signature_str)?;

    // 使用 JsonParsed 格式
    let tx_detail = client.get_transaction(&signature, UiTransactionEncoding::JsonParsed).await;

    match tx_detail {
        Ok(tx) => {
            if let Some(meta) = tx.transaction.meta {
                // 确保数据完整
                if meta.pre_balances.len() == 0 || meta.post_balances.len() == 0 {
                    return Ok(());
                }

                let pre_bal = meta.pre_balances[0];
                let post_bal = meta.post_balances[0];

                let diff_lamports = (pre_bal as i64 - post_bal as i64).abs();
                let sol_amount = diff_lamports as f64 / 1_000_000_000.0;

                // 阈值测试：0.1 SOL
                if sol_amount > 0.1 {
                    println!("🐋 捕获! https://solscan.io/tx/{}", signature_str);
                    println!("   💰 {:.4} SOL (Account0 变动)", sol_amount);
                    println!("-------------------------------------------");
                }
            }
        }
        Err(e) => {
            // 如果是 "Transaction X not found"，说明 RPC 还没同步完这笔刚发生的交易
            // 在生产环境中，我们通常会在这里 sleep 500ms 然后重试一次
            // 这里为了简单先忽略
            // eprintln!("RPC 查询过早: {}", e);
        }
    }
    Ok(())
}