import { Navigate, Route, Routes } from 'react-router-dom';
import HomePage from './pages/HomePage';
import ReaderPage from './pages/ReaderPage';
import { useReaderStore } from './store';

function ReaderGate() {
  const manga = useReaderStore((state) => state.manga);
  const volume = useReaderStore((state) => state.volume);

  if (!manga || !volume) {
    return <Navigate to="/" replace />;
  }

  return <ReaderPage />;
}

export default function App() {
  return (
    <Routes>
      <Route path="/" element={<HomePage />} />
      <Route path="/reader" element={<ReaderGate />} />
    </Routes>
  );
}
