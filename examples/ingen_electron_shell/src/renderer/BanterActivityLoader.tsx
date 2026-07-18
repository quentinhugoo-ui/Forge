const LOADER_SIZE = 72;
const BANTER_LOADER_BOX_IDS = [1, 2, 3, 4, 5, 6, 7, 8, 9];

export function BanterActivityLoader() {
  return (
    <span className="banterActivityLoader" aria-hidden="true">
      <span className="banterActivityLoader__stage">
        {BANTER_LOADER_BOX_IDS.map((id) => {
          const isThird = id % 3 === 0;
          return (
            <span
              className="banterActivityLoader__cell"
              key={id}
              style={{
                width: 20,
                height: 20,
                marginRight: isThird ? 0 : 6,
                marginBottom: isThird ? 6 : 0,
                ...(id === 9 ? { marginBottom: 0 } : {}),
                animation: `banterActivityMoveBox-${id} 4s infinite`
              }}
            >
              <span
                className="banterActivityLoader__box"
                style={{
                  marginLeft: id === 1 || id === 4 ? 26 : 0,
                  marginTop: id === 3 ? 52 : 0
                }}
              />
            </span>
          );
        })}
      </span>
    </span>
  );
}