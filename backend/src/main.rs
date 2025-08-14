use axum::{
    extract::{State, Query},
    http::{Method, StatusCode},
    response::Json as JsonResponse,
    routing::get,
    Router,
};
use clap::Parser;
use common::mahjong::parse_hand_str;
use serde::Serialize;
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, Level};
use tracing_subscriber;

mod analysis;
mod flat_file_vec_pool;

use analysis::SharedHandAnalyzer;

use crate::analysis::{MentsuAnalysis, TsumoAnalysis};

/// コマンドライン引数
#[derive(Parser, Debug)]
#[command(author, version, about = "麻雀手牌分析サーバー", long_about = None)]
struct Args {
    /// HandConverterファイルのパス
    #[arg(long)]
    conv_path: String,

    /// 13枚用ツモ率データファイルのパス
    #[arg(long)]
    tsumo_13_path: String,

    /// 14枚用ツモ率データファイルのパス
    #[arg(long)]
    tsumo_14_path: String,

    /// 13枚用メトリクスデータファイルのパス
    #[arg(long)]
    metrics_13_path: String,

    /// 14枚用メトリクスデータファイルのパス
    #[arg(long)]
    metrics_14_path: String,

    /// ファイルプールの最大サイズ
    #[arg(long, default_value = "128")]
    max_pool_size: usize,
}

// アプリケーションの状態
#[derive(Clone)]
struct AppState {
    analyzer: SharedHandAnalyzer,
}

// エラーレスポンス
#[derive(Serialize, Debug)]
struct ErrorResponse {
    error: String,
    code: String,
    message: String,
}

// 手牌分析のハンドラー
async fn analyze_tsumo(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<JsonResponse<TsumoAnalysis>, (StatusCode, JsonResponse<ErrorResponse>)> {
    // クエリパラメータから手牌を取得
    let hand_string = match params.get("hand") {
        Some(hand) => hand,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                JsonResponse(ErrorResponse {
                    error: "Missing 'hand' parameter".to_string(),
                    code: "BAD_REQUEST".to_string(),
                    message: "Missing 'hand' parameter".to_string(),
                }),
            ));
        }
    };
    
    info!("Received tsumo analysis request: hand={}", hand_string);
    
    // 手牌文字列をパース
    let hand = match parse_hand_str(hand_string) {
        Ok(hand) => hand,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                JsonResponse(ErrorResponse {
                    error: "Invalid hand format".to_string(),
                    code: "BAD_REQUEST".to_string(),
                    message: format!("Invalid hand format: {}", e),
                }),
            ));
        }
    };
    
    // 共有分析エンジンを使用して手牌を分析
    let analysis = match state.analyzer.analyze_tsumo(&hand).await {
        Ok(analysis) => analysis,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(ErrorResponse {
                    error: "Failed to analyze tsumo".to_string(),
                    code: "INTERNAL_SERVER_ERROR".to_string(),
                    message: format!("Failed to analyze tsumo: {}", e),
                }),
            ));
        }
    };
    
    info!("Tsumo analysis completed: hand={}", hand_string);

    Ok(JsonResponse(analysis))
}

async fn analyze_mentsu(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<JsonResponse<MentsuAnalysis>, (StatusCode, JsonResponse<ErrorResponse>)> {
    // クエリパラメータから手牌を取得
    let hand_string = match params.get("hand") {
        Some(hand) => hand,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                JsonResponse(ErrorResponse {
                    error: "Missing 'hand' parameter".to_string(),
                    code: "BAD_REQUEST".to_string(),
                    message: "Missing 'hand' parameter".to_string(),
                }),
            ));
        }
    };

    // クエリパラメータから残り巡数を取得
    let draws_left_str = match params.get("draws_left") {
        Some(draws_left) => draws_left,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                JsonResponse(ErrorResponse {
                    error: "Missing 'draws_left' parameter".to_string(),
                    code: "BAD_REQUEST".to_string(),
                    message: "Missing 'draws_left' parameter".to_string(),
                }),
            ));
        }
    };

    info!("Received mentsu analysis request: hand={}, draws_left={}", hand_string, draws_left_str);

    let draws_left = match draws_left_str.parse::<usize>() {
        Ok(draws_left) => draws_left,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                JsonResponse(ErrorResponse {
                    error: "Invalid draws_left format".to_string(),
                    code: "BAD_REQUEST".to_string(),
                    message: format!("Invalid draws_left format: {}", e),
                }),
            ));
        }
    };
    
    // 手牌文字列をパース
    let hand = match parse_hand_str(hand_string) {
        Ok(hand) => hand,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                JsonResponse(ErrorResponse {
                    error: "Invalid hand format".to_string(),
                    code: "BAD_REQUEST".to_string(),
                    message: format!("Invalid hand format: {}", e),
                }),
            ));
        }
    };

    // 共有分析エンジンを使用して手牌を分析
    let analysis = match state.analyzer.analyze_mentsu(&hand, draws_left).await {
        Ok(analysis) => analysis,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(ErrorResponse {
                    error: "Failed to analyze mentsu".to_string(),
                    code: "INTERNAL_SERVER_ERROR".to_string(),
                    message: format!("Failed to analyze mentsu: {}", e),
                }),
            ));
        }
    };
    
    info!("Mentsu analysis completed: hand={}, draws_left={}", hand_string, draws_left_str);

    Ok(JsonResponse(analysis))

}



// ヘルスチェックエンドポイント
async fn health_check() -> &'static str {
    "OK"
}

fn main() {
    // コマンドライン引数を解析
    let args = Args::parse();

    // ログの初期化
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    // 論理コア数を取得
    let worker_threads = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4); // フォールバック値として4を使用

    info!("Starting tsumo probability backend server...");
    info!("Using multi-threaded runtime with {} worker threads", worker_threads);
    info!("Configuration: {:?}", args);

    // Tokioランタイムを手動で構築
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .enable_all()
        .build()
        .expect("Failed to build Tokio runtime");

    // 非同期メイン処理を実行
    rt.block_on(async_main(args));
}

async fn async_main(args: Args) {
    // 共有分析エンジンを初期化
    let analyzer = match SharedHandAnalyzer::new(
        &args.conv_path,
        &args.tsumo_13_path,
        &args.tsumo_14_path,
        &args.metrics_13_path,
        &args.metrics_14_path,
        args.max_pool_size,
    ) {
        Ok(analyzer) => {
            info!("Hand analyzer initialized successfully");
            analyzer
        }
        Err(e) => {
            eprintln!("Failed to initialize hand analyzer: {}", e);
            std::process::exit(1);
        }
    };

    // アプリケーション状態を作成
    let state = AppState { analyzer };

    // CORS設定
    let cors = CorsLayer::new()
        .allow_methods([Method::POST, Method::GET])
        .allow_origin(Any);

    // ルーターの設定（状態を共有）
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/analyze-tsumo", get(analyze_tsumo))
        .route("/analyze-mentsu", get(analyze_mentsu))
        .layer(cors)
        .with_state(state);

    // サーバーの起動
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    info!("Server listening on http://127.0.0.1:3000");
    
    axum::serve(listener, app).await.unwrap();
}