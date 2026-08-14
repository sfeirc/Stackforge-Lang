FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --bin stackforge

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=build /src/target/release/stackforge ./
COPY --from=build /src/examples ./examples
ENTRYPOINT ["./stackforge"]
CMD ["examples/demo.sf"]
