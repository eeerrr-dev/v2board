import { useEffect } from 'react';
import { useNavigate } from 'react-router';
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
    return (
      <div
        className="custom-html-style"
        dangerouslySetInnerHTML={{ __html: sanitizeLegacyHtml(decodeHomepage(homepage)) }}
      />
    );
  }

  return (
    <div className="text-center pt-[50px]">
      <a href="https://github.com/wyx2685/v2board">v2board</a> is best.
    </div>
  );
}
