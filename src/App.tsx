import {  Route, Routes } from "react-router-dom";
import "./App.css";
import Lyrics from "./components/Lyrics";
import Settings from "./components/Settings";

function App() {
  return (
    <Routes>
      <Route path="/" element={<Lyrics />} />
      <Route path="/settings" element={<Settings />} />
    </Routes>
  );
}

export default App;

