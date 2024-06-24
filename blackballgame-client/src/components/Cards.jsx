export default function Cards({ cards, pushCardsToHand }) {
  console.log("jere/ cards: ", cards);

  return (
    <div className="flex items-center justify-center">
      <div className="grid w-full max-w-5xl grid-cols-[1fr_2fr_1fr] gap-6 px-4 md:px-6">
        <div className="flex flex-col items-center gap-4">
          <div className="grid grid-cols-5 gap-4">
            {cards.map((card) => {
              return (
                <div
                  className="flex h-[200px] w-[140px] items-center justify-center rounded-lg bg-white shadow-lg dark:bg-gray-800"
                  onMouseDown={(evt) =>
                    pushCardsToHand((curr) => [...curr, card])
                  }
                >
                  <div className="flex flex-col items-center gap-2">
                    <span className="text-4xl font-bold">{card.value}</span>
                    <span className="text-2xl font-medium">{card.suit}</span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}
