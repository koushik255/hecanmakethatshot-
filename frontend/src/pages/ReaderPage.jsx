import { useEffect, useMemo, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { getManifest } from '../api';
import { useReaderStore } from '../store';

function getCurrentDisplay(manifest, stepIndex) {
  if (!manifest || manifest.steps.length === 0) {
    return null;
  }

  const step = manifest.steps[stepIndex] ?? manifest.steps[0];
  if (!step) return null;

  if (step.kind === 'spread') {
    return {
      kind: 'spread',
      right: manifest.pages[step.right],
      left: manifest.pages[step.left],
    };
  }

  return {
    kind: 'single',
    page: manifest.pages[step.page],
  };
}

export default function ReaderPage() {
  const navigate = useNavigate();
  const manga = useReaderStore((state) => state.manga);
  const volume = useReaderStore((state) => state.volume);
  const volumes = useReaderStore((state) => state.volumes);
  const manifest = useReaderStore((state) => state.manifest);
  const step = useReaderStore((state) => state.step);
  const setManifest = useReaderStore((state) => state.setManifest);
  const setStep = useReaderStore((state) => state.setStep);
  const nextStep = useReaderStore((state) => state.nextStep);
  const prevStep = useReaderStore((state) => state.prevStep);
  const nextVolume = useReaderStore((state) => state.nextVolume);

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [topbarHidden, setTopbarHidden] = useState(false);
  const [alignment, setAlignment] = useState('center');

  useEffect(() => {
    if (!manga || !volume) {
      navigate('/');
      return;
    }

    let active = true;
    setLoading(true);
    setError('');

    getManifest(manga, volume)
      .then((data) => {
        if (!active) return;
        setManifest(data);
        setStep(0);
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
  }, [manga, volume, navigate, setManifest, setStep]);

  const display = useMemo(() => getCurrentDisplay(manifest, step), [manifest, step]);
  const currentVolumeIndex = volumes.findIndex((item) => item.number === volume);
  const hasNextVolume = currentVolumeIndex !== -1 && currentVolumeIndex + 1 < volumes.length;

  useEffect(() => {
    function onKeyDown(event) {
      if (event.code === 'Space' || event.code === 'ArrowRight') {
        event.preventDefault();
        nextStep();
      } else if (event.code === 'ArrowLeft') {
        event.preventDefault();
        prevStep();
      } else if (event.key === 'h' || event.key === 'H') {
        event.preventDefault();
        setTopbarHidden((value) => !value);
      }
    }

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [nextStep, prevStep]);

  function handleNextVolume() {
    if (!hasNextVolume) return;
    nextVolume();
  }

  if (!manga || !volume) {
    return null;
  }

  return (
    <div className="reader-page">
      <button
        type="button"
        className="topbar-toggle"
        onClick={() => setTopbarHidden((value) => !value)}
      >
        {topbarHidden ? 'Show Bar' : 'Hide Bar'}
      </button>

      {!topbarHidden ? (
        <div className="reader-topbar">
          <div className="reader-meta">
            {manga} · volume {volume}
            {manifest ? ` · step ${step + 1}/${manifest.steps.length}` : ''}
          </div>
          <div className="reader-actions">
            <Link to="/" className="secondary-button link-button">
              Home
            </Link>
            <button
              type="button"
              className={`secondary-button ${alignment === 'left' ? 'active' : ''}`}
              onClick={() => setAlignment('left')}
            >
              Left Align
            </button>
            <button
              type="button"
              className={`secondary-button ${alignment === 'center' ? 'active' : ''}`}
              onClick={() => setAlignment('center')}
            >
              Center
            </button>
            <button
              type="button"
              className={`secondary-button ${alignment === 'right' ? 'active' : ''}`}
              onClick={() => setAlignment('right')}
            >
              Right Align
            </button>
            <button
              type="button"
              className="reader-button"
              disabled={!hasNextVolume}
              onClick={handleNextVolume}
            >
              Next Volume
            </button>
          </div>
        </div>
      ) : null}

      <div className="reader-stage">
        {loading ? <div className="reader-message">Loading…</div> : null}
        {error ? <pre className="error-box">{error}</pre> : null}

        {!loading && !error && display?.kind === 'spread' ? (
          <div className={`spread-view ${alignment === 'right' ? 'right-aligned' : ''} ${alignment === 'left' ? 'left-aligned' : ''}`}>
            <img
              src={display.left?.image_url}
              alt="Left page"
              className="spread-photo"
              draggable="false"
            />
            <img
              src={display.right?.image_url}
              alt="Right page"
              className="spread-photo"
              draggable="false"
            />
          </div>
        ) : null}

        {!loading && !error && display?.kind === 'single' ? (
          <div className={`solo-view ${alignment === 'right' ? 'right-aligned' : ''} ${alignment === 'left' ? 'left-aligned' : ''}`}>
            <img
              src={display.page?.image_url}
              alt="Solo page"
              className={display.page?.is_landscape ? 'solo-photo solo-photo-landscape' : 'solo-photo'}
              draggable="false"
            />
          </div>
        ) : null}
      </div>
    </div>
  );
}
