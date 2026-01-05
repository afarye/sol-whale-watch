use dotenv::dotenv;
use solana_client::nonblocking::pubsub_client::PubsubClient;
use solana_client::rpc_config::{RpcTransactionLogsConfig, RpcTransactionLogsFilter};
use solana_client::nonblocking::rpc_client::RpcClient;
use tokio::sync::mpsc;
use solana_sdk::commitment_config::CommitmentConfig;
use futures::StreamExt;
use solana_transaction_status::UiTransactionEncoding;
use solana_sdk::signature::Signature;
use std::env;
use std::str::FromStr;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    println!("🚀 启动 Solana 巨鲸监控者 (最终完整版)...");

    let ws_url = env::var("WS_URL").expect("WS_URL 未设置");
    let rpc_url = env::var("RPC_URL").expect("RPC_URL 未设置");
    
    // 检查 TG 配置，如果没有配置只会打印警告，不会崩溃
    if env::var("TELEGRAM_TOKEN").is_err() {
        println!("⚠️ 未检测到 TELEGRAM_TOKEN，报警功能将不可用");
    }

    let (tx, mut rx) = mpsc::channel::<String>(100);

    // --- 后台消费者 ---
    tokio::spawn(async move {
        println!("👨‍🔧 后台调度中心已就位...");
        let rpc_client = RpcClient::new(rpc_url);
        let client_arc = Arc::new(rpc_client);

        while let Some(signature) = rx.recv().await {
            let client_ref = client_arc.clone();
            tokio::spawn(async move {
                // 处理交易，并不再关心返回值，只负责跑
                if let Err(_e) = process_transaction(client_ref, signature).await {
                    // 生产环境下这里可以用 log crate 记录到文件
                    // eprintln!("❌ Error: {}", e);
                }
            });
        }
    });

    // --- 前端生产者 ---
    println!("📡 连接 WebSocket...");
    let pubsub_client = PubsubClient::new(&ws_url).await?;
    let filter = RpcTransactionLogsFilter::Mentions(vec!["11111111111111111111111111111111".to_string()]);
    let config = RpcTransactionLogsConfig {
        commitment: Some(CommitmentConfig::processed()),
    };
    let (mut stream, _unsub) = pubsub_client.logs_subscribe(filter, config).await?;

    println!("🎧 监听中... (等待巨鲸出现)");

    while let Some(response) = stream.next().await {
        let logs = response.value;
        if logs.err.is_some() { continue; }
        if let Err(_) = tx.send(logs.signature.clone()).await { break; }
    }

    Ok(())
}

async fn process_transaction(client: Arc<RpcClient>, signature_str: String) -> anyhow::Result<()> {
    let signature = Signature::from_str(&signature_str)?;
    let tx_detail = client.get_transaction(&signature, UiTransactionEncoding::JsonParsed).await;

    if let Ok(tx) = tx_detail {
        if let Some(meta) = tx.transaction.meta {
            if meta.pre_balances.len() == 0 || meta.post_balances.len() == 0 { return Ok(()); }

            let pre_bal = meta.pre_balances[0];
            let post_bal = meta.post_balances[0];
            let diff_lamports = (pre_bal as i64 - post_bal as i64).abs();
            let sol_amount = diff_lamports as f64 / 1_000_000_000.0;

            // 为了测试，我们可以把阈值设低一点，比如 0.1 SOL
            if sol_amount > 0.1 {
                let msg = format!(
                    "🐋 <b>巨鲸警报!</b>\n\n💰 <b>金额:</b> {:.2} SOL\n🔗 <a href=\"https://solscan.io/tx/{}\">查看交易详情</a>\n📉 余额变化: {:.2} -> {:.2}",
                    sol_amount, signature_str, 
                    pre_bal as f64 / 1e9, post_bal as f64 / 1e9
                );

                println!("--------\n{}\n--------", msg); // 终端也打印一份

                // 🔥 发送报警 (Fire and forget: 不用等它发送成功，发出去就行)
                // 这里我们不需要 .await? 阻塞当前函数，但因为我们需要它是异步的，
                // 所以直接调用，让它在当前任务里跑完即可。
                send_telegram_alert(msg).await;
            }
        }
    }
    Ok(())
}

// --- 5. 新增：Telegram 报警模块 ---
async fn send_telegram_alert(message: String) {
    let token = match env::var("TELEGRAM_TOKEN") {
        Ok(t) => t,
        Err(_) => return,
    };
    let chat_id = match env::var("TELEGRAM_CHAT_ID") {
        Ok(id) => id,
        Err(_) => return,
    };

    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);

    // 打印调试信息：看看我们到底发了什么 ID
    // println!("DEBUG: 正在发送给 Chat ID: '{}'", chat_id); 

    let params = serde_json::json!({
        "chat_id": chat_id, // 这里的 chat_id 如果包含空格或换行符会导致 400
        "text": message,
        "parse_mode": "HTML",
        "disable_web_page_preview": true
    });

    // 强制使用你设置的代理端口 (根据你之前的命令是 7897)
    let proxy = reqwest::Proxy::all("http://127.0.0.1:7897").unwrap();
    let client = reqwest::Client::builder()
        .proxy(proxy)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    
    match client.post(url).json(&params).send().await {
        Ok(res) => {
            if !res.status().is_success() {
                eprintln!("⚠️ Telegram 发送失败: Status {}", res.status());
                // 🔥 新增：打印具体的错误响应体，这能告诉我们到底是哪里错了
                if let Ok(text) = res.text().await {
                    eprintln!("❌ 错误原因: {}", text);
                }
            } else {
                println!("✅ Telegram 报警发送成功!");
            }
        },
        Err(e) => eprintln!("⚠️ Telegram 网络错误: {}", e),
    }
}