import DetailWindowView, { parseDetailModuleFromHash } from "./components/DetailWindowView";
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
