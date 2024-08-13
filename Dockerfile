# does work with libssl-dev
# possible inspo: https://github.com/atomicdata-dev/atomic-server/blob/745511acfab17d8155973db9619bc006c9f943b7/Earthfile#L4
FROM debian:bookworm-slim 
ARG ENVIRONMENT=production
RUN apt-get update && apt-get install -y libssl-dev

# FROM scratch
# FROM gcr.io/distroless/cc
# RUN apt-get update && apt-get install -y libssl-dev
WORKDIR /myapp
COPY +build/target/blackballgame-server /myapp
EXPOSE 8080
CMD ["./blackballgame-server"]