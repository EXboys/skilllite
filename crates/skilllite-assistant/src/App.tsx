import DetailWindowView, { parseDetailModuleFromHash } from "./components/DetailWindowView";
import MainLayout from "./components/MainLayout";

function App() {
  const detailModule = parseDetailModuleFromHash();
  if (detailModule) {
    return <DetailWindowView />;
  }
  return <MainLayout />;
}

export default App;
