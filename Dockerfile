FROM rust:1.76 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libgomp1 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/afo-ai .
COPY qwen2.5-0.5b-instruct-q4_k_m.gguf .

ENV PORT=8080
EXPOSE 8080
CMD ["./afo-ai"]