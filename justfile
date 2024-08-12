all:
    earthly --no-sat +all

build:
    earthly --no-sat +build
    # earthly +build
    
buildfe:
    # cd blackballgame-client && npm run build
    cd bb-admin && dx build --release

lint:
    earthly --no-sat +lint

docker:
    just buildfe
    earthly --no-sat +docker

push:
    # echo $DOCKERHUB_TOKEN | docker login --username "$DOCKERHUB_USERNAME" --password-stdin
    # docker login --username "$DOCKERHUB_USERNAME" --password "$DOCKERHUB_TOKEN"
    earthly --no-sat --push +docker

test:
    earthly --no-sat +test

rund: 
    docker run --env-file ./blackballgame-server/.env -p 8080:8080 -it jerecan/blackballgame:blackballgame-server