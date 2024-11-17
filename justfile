all:
    earthly --no-sat +all

build:
    earthly --no-sat +build
    # earthly +build
    
buildfe:
    cd bb-admin && dx build --release

lint:
    earthly --no-sat +lint

docker:
    earthly --no-sat +docker

push:
    # echo $DOCKERHUB_TOKEN | docker login --username "$DOCKERHUB_USERNAME" --password-stdin
    # docker login --username "$DOCKERHUB_USERNAME" --password "$DOCKERHUB_TOKEN"
    earthly --no-sat --push +docker

test:
    earthly --no-sat +test

rund: 
    docker run --env-file ./bb-server/.env -p 8080:8080 -it jerecan/blackballgame:blackballgame-server