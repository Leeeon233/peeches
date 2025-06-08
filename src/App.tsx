import { Route, Routes } from "react-router-dom";
import "./App.css";
import Lyrics from "./components/Lyrics";
import Settings from "./components/Settings";
import History from "./components/History";

function App() {
  return (
    <Routes>
      <Route path="/" element={<Lyrics />} />
      <Route path="/settings" element={<Settings />} />
      <Route path="/history" element={<History />} />
    </Routes>
  );
}

export default App;

