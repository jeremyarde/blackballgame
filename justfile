all:
    earthly --no-sat +all

fe:
    cd bb-admin && export STAGE="test" && dx serve --port 5173 --hot-reload true

be:
    cd bb-server && cargo run

tailwind:
    cd bb-admin && npx tailwindcss -i ./input.css -o ./assets/tailwind.css --watch


build:
    earthly --no-sat +build
    # earthly +build
    
buildfe:
    cd bb-admin && dx build --release

deployfe:
    cd bb-admin && dx build --release
    target/dx/bb-admin/release/web/public

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