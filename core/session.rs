use crate::core::traits::agent::MessageContext;

pub fn generate_session_key(channel: &str, ctx: &MessageContext) -> String {
    match channel {
        "feishu" | "telegram" | "discord" => {
            if let Some(group) = &ctx.group_id {
                format!(
                    "{}:group:{}:thread:{}",
                    channel,
                    group,
                    if ctx.thread_id.is_empty() { "main" } else { &ctx.thread_id }
                )
            } else {
                format!("{}:dm:{}", channel, ctx.user_id)
            }
        }
        _ => format!("{}:{}", channel, ctx.user_id),
    }
}
