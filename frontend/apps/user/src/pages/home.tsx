import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { getLegacySettings } from '@/lib/legacy-settings';
import { sanitizeLegacyHtml } from '@/lib/sanitize-html';

function decodeHomepage(value: string) {
  return decodeURI(window.atob(value));
}

export default function HomePage() {
  const navigate = useNavigate();
  const homepage = getLegacySettings().homepage;

  useEffect(() => {
    if (!homepage) navigate('/login');
  }, [homepage, navigate]);

  if (homepage) {
    return <div dangerouslySetInnerHTML={{ __html: sanitizeLegacyHtml(decodeHomepage(homepage)) }} />;
  }

  return (
    <div style={{ textAlign: 'center', paddingTop: 50 }}>
      <a href="https://github.com/wyx2685/v2board">v2board</a> is best.
    </div>
  );
}
