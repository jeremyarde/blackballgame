- don't allow starting game with less than 2 people
- show lobbies you can join
- private/public lobbies
- show players in your lobby
- better displayer for your own bids (under hand?)
- show your wins under your hand
- hover for extra game details from a tab on the side?

game stuff
- allow quitting a game
- need to drop games that aren't active
- disconnect issues kind of?
- split messages between game and system so you can get active games, join lobbies, etc

curl -X POST 'localhost:8080/rooms' \
-H 'Content-Type: application/json' \
-d '{
    "lobby_code": "testing"
}'


curl -X POST 'localhost:8080/rooms' \
-H 'Content-Type: application/json' \
-d '{
    "lobby_code": "test1"
}'