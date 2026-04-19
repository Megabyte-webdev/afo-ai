FROM rust:1.80 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libgomp1 wget && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/afo-ai .

# optional: keep if you insist on build-time download
RUN wget -O qwen.gguf \
    https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q4_k_m.gguf

ENV PORT=8080
ENV MODEL_PATH=./qwen.gguf

EXPOSE 8080
CMD ["./afo-ai"]