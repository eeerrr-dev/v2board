import { useLocation } from 'react-router';
import { CouponsView } from './coupons-view';
import { GiftcardsView } from './giftcards-view';

export default function CouponsPage() {
  const location = useLocation();
  if (location.pathname === '/giftcard') return <GiftcardsView />;
  return <CouponsView />;
}
