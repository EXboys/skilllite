import DetailWindowView from "./components/DetailWindowView";
import { parseDetailModuleFromHash } from "./utils/detailWindow";
import MainLayout from "./components/MainLayout";
import { ToastHost } from "./components/ToastHost";

function App() {
  const detailModule = parseDetailModuleFromHash();
  return (
    <>
      {detailModule ? <DetailWindowView /> : <MainLayout />}
      <ToastHost />
    </>
  );
}

export default App;
