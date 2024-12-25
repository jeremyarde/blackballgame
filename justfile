all:
    earthly --no-sat +all

fe:
    cd bb-admin && export STAGE="test" && dx serve --port 5173 --hot-reload true

be:
    cd bb-server && cargo run

tw:
    cd bb-admin && npx tailwindcss -i ./input.css -o ./public/tailwind.css --watch


build:
    earthly --no-sat +build
    # earthly +build
    
buildfe:
    cd bb-admin && dx build --release

deployfe:
    cd bb-admin && npx tailwindcss -i ./input.css -o ./public/tailwind.css
    cd bb-admin && export STAGE=production && dx build --release
    # git checkout deploy
    # git add -f target/dx/bb-admin/release/web/public/*
    # git commit -m "deploy"
    # git push origin deploy
    mkdir -p docs
    cp -r target/dx/bb-admin/release/web/public/* docs

deploybe:
    fly deploy
    
lint:
    earthly --no-sat +lint

docker:
    earthly --no-sat +docker

push:
    echo $DOCKERHUB_TOKEN | docker login --username "$DOCKERHUB_USERNAME" --password-stdin
    docker login --username "$DOCKERHUB_USERNAME" --password "$DOCKERHUB_TOKEN"
    earthly --no-sat --push +docker

test:
    earthly --no-sat +test

rund: 
    docker run --env-file ./bb-server/.env -p 8080:8080 -it jerecan/blackballgame:blackballgame-server