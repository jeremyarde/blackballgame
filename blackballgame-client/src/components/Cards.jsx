export default function Cards({ cards }) {
  return (
    <div className="flex h-screen w-full items-center justify-center bg-gray-100 dark:bg-gray-950">
      <div className="grid w-full max-w-5xl grid-cols-[1fr_2fr_1fr] gap-6 px-4 md:px-6">
        <div className="flex flex-col items-center gap-4">
          <div className="grid grid-cols-3 gap-4">
            {cards.map((card) => {
              <div>
                <div className="flex h-[200px] w-[140px] items-center justify-center rounded-lg bg-white shadow-lg dark:bg-gray-800">
                  <div className="flex flex-col items-center gap-2">
                    <span className="text-4xl font-bold">A</span>
                    <span className="text-2xl font-medium">♥</span>
                  </div>
                </div>
                <div className="flex h-[200px] w-[140px] items-center justify-center rounded-lg bg-white shadow-lg dark:bg-gray-800">
                  <div className="flex flex-col items-center gap-2">
                    <span className="text-4xl font-bold">K</span>
                    <span className="text-2xl font-medium">♦</span>
                  </div>
                </div>
                <div className="flex h-[200px] w-[140px] items-center justify-center rounded-lg bg-white shadow-lg dark:bg-gray-800">
                  <div className="flex flex-col items-center gap-2">
                    <span className="text-4xl font-bold">7</span>
                    <span className="text-2xl font-medium">♣</span>
                  </div>
                </div>
              </div>;
            })}
          </div>
          <div className="flex items-center gap-2">
            <input
              className="flex h-10 rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background file:border-0 file:bg-transparent file:text-sm file:font-medium placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 w-24"
              placeholder="Enter bid"
              type="number"
            />
            <button className="inline-flex items-center justify-center whitespace-nowrap text-sm font-medium ring-offset-background transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 bg-primary text-primary-foreground hover:bg-primary/90 h-9 rounded-md px-3">
              Play Card
            </button>
          </div>
        </div>
        <div className="flex flex-col items-center gap-4">
          <div className="grid grid-cols-5 gap-4">
            <div className="flex h-[200px] w-[140px] items-center justify-center rounded-lg bg-white shadow-lg dark:bg-gray-800">
              <div className="flex flex-col items-center gap-2">
                <span className="text-4xl font-bold">2</span>
                <span className="text-2xl font-medium">♠</span>
              </div>
            </div>
            <div className="flex h-[200px] w-[140px] items-center justify-center rounded-lg bg-white shadow-lg dark:bg-gray-800">
              <div className="flex flex-col items-center gap-2">
                <span className="text-4xl font-bold">5</span>
                <span className="text-2xl font-medium">♥</span>
              </div>
            </div>
            <div className="flex h-[200px] w-[140px] items-center justify-center rounded-lg bg-white shadow-lg dark:bg-gray-800">
              <div className="flex flex-col items-center gap-2">
                <span className="text-4xl font-bold">Q</span>
                <span className="text-2xl font-medium">♦</span>
              </div>
            </div>
            <div className="flex h-[200px] w-[140px] items-center justify-center rounded-lg bg-white shadow-lg dark:bg-gray-800">
              <div className="flex flex-col items-center gap-2">
                <span className="text-4xl font-bold">J</span>
                <span className="text-2xl font-medium">♣</span>
              </div>
            </div>
            <div className="flex h-[200px] w-[140px] items-center justify-center rounded-lg bg-white shadow-lg dark:bg-gray-800">
              <div className="flex flex-col items-center gap-2">
                <span className="text-4xl font-bold">10</span>
                <span className="text-2xl font-medium">♠</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
