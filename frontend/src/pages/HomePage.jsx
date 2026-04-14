import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { getMangaList, getVolumes } from '../api';
import { useReaderStore } from '../store';

export default function HomePage() {
  const navigate = useNavigate();
  const setSelection = useReaderStore((state) => state.setSelection);
  const savedManga = useReaderStore((state) => state.manga);
  const savedVolume = useReaderStore((state) => state.volume);

  const [manga, setManga] = useState([]);
  const [selectedManga, setSelectedManga] = useState(savedManga);
  const [volumes, setVolumes] = useState([]);
  const [selectedVolume, setSelectedVolume] = useState(savedVolume ?? 1);
  const [loading, setLoading] = useState(true);
  const [volumeLoading, setVolumeLoading] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    let active = true;

    getMangaList()
      .then((items) => {
        if (!active) return;
        setManga(items);

        const initialManga = savedManga ?? items[0]?.name ?? null;
        setSelectedManga(initialManga);
      })
      .catch((err) => {
        if (!active) return;
        setError(err.message);
      })
      .finally(() => {
        if (active) setLoading(false);
      });

    return () => {
      active = false;
    };
  }, [savedManga]);

  useEffect(() => {
    if (!selectedManga) {
      setVolumes([]);
      return;
    }

    let active = true;
    setVolumeLoading(true);
    setError('');

    getVolumes(selectedManga)
      .then((items) => {
        if (!active) return;
        setVolumes(items);

        const nextVolume = items.some((item) => item.number === selectedVolume)
          ? selectedVolume
          : items[0]?.number ?? 1;
        setSelectedVolume(nextVolume);
      })
      .catch((err) => {
        if (!active) return;
        setError(err.message);
        setVolumes([]);
      })
      .finally(() => {
        if (active) setVolumeLoading(false);
      });

    return () => {
      active = false;
    };
  }, [selectedManga]);

  function openReader() {
    if (!selectedManga || !selectedVolume) return;
    setSelection({ manga: selectedManga, volume: selectedVolume, volumes });
    navigate('/reader');
  }

  return (
    <div className="home-page">
      <div className="home-panel">
        <h1 className="home-title">Choose a Manga</h1>

        {loading ? <div>Loading manga…</div> : null}
        {error ? <pre className="error-box">{error}</pre> : null}

        <div className="home-layout">
          <div className="home-column">
            <div className="section-title">Manga</div>
            <div className="manga-list">
              {manga.map((item) => (
                <button
                  key={item.name}
                  type="button"
                  className={`list-button ${selectedManga === item.name ? 'active' : ''}`}
                  onClick={() => setSelectedManga(item.name)}
                >
                  {item.name}
                </button>
              ))}
            </div>
          </div>

          <div className="home-column">
            <div className="section-title">Volumes</div>
            {volumeLoading ? <div>Loading volumes…</div> : null}
            <div className="manga-list">
              {volumes.map((item) => (
                <button
                  key={item.number}
                  type="button"
                  className={`list-button ${selectedVolume === item.number ? 'active' : ''}`}
                  onClick={() => setSelectedVolume(item.number)}
                >
                  {item.label}
                </button>
              ))}
            </div>
          </div>
        </div>

        <div className="home-actions">
          <button type="button" className="reader-button" onClick={openReader}>
            Open Reader
          </button>
        </div>
      </div>
    </div>
  );
}
