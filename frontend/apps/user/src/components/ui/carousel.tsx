import useEmblaCarousel, { type UseEmblaCarouselType } from 'embla-carousel-react';
import {
  type ComponentProps,
  type KeyboardEvent,
  createContext,
  useCallback,
  useContext,
  useEffect,
} from 'react';
import { cn } from '@/lib/cn';

export type CarouselApi = UseEmblaCarouselType[1];
type UseCarouselParameters = Parameters<typeof useEmblaCarousel>;
type CarouselOptions = UseCarouselParameters[0];
type CarouselPlugin = UseCarouselParameters[1];
type CarouselOrientation = 'horizontal' | 'vertical';

interface CarouselProps {
  opts?: CarouselOptions;
  plugins?: CarouselPlugin;
  orientation?: CarouselOrientation;
  /** Receives the embla api once ready, e.g. to drive external dot navigation. */
  setApi?: (api: CarouselApi) => void;
}

interface CarouselContextValue {
  carouselRef: UseEmblaCarouselType[0];
  api: CarouselApi;
  orientation: CarouselOrientation;
}

const CarouselContext = createContext<CarouselContextValue | null>(null);

export function useCarousel() {
  const context = useContext(CarouselContext);
  if (!context) {
    throw new Error('useCarousel must be used within a <Carousel>');
  }
  return context;
}

/**
 * Embla-backed carousel adapted to the shadcn island house style (data-slot,
 * cn, function components). Provides drag/swipe and left/right keyboard nav out
 * of the box; consumers wire their own dot/thumb navigation via `setApi`.
 */
export function Carousel({
  orientation = 'horizontal',
  opts,
  setApi,
  plugins,
  className,
  children,
  ...props
}: ComponentProps<'div'> & CarouselProps) {
  const [carouselRef, api] = useEmblaCarousel(
    { ...opts, axis: orientation === 'horizontal' ? 'x' : 'y' },
    plugins,
  );

  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      if (event.key === 'ArrowLeft') {
        event.preventDefault();
        api?.scrollPrev();
      } else if (event.key === 'ArrowRight') {
        event.preventDefault();
        api?.scrollNext();
      }
    },
    [api],
  );

  useEffect(() => {
    if (api && setApi) setApi(api);
  }, [api, setApi]);

  return (
    <CarouselContext.Provider value={{ carouselRef, api, orientation }}>
      <div
        data-slot="carousel"
        role="region"
        aria-roledescription="carousel"
        className={cn('relative', className)}
        onKeyDownCapture={handleKeyDown}
        {...props}
      >
        {children}
      </div>
    </CarouselContext.Provider>
  );
}

export function CarouselContent({ className, ...props }: ComponentProps<'div'>) {
  const { carouselRef, orientation } = useCarousel();
  return (
    <div ref={carouselRef} data-slot="carousel-content" className="overflow-hidden">
      <div
        className={cn(
          'flex',
          orientation === 'horizontal' ? '-ml-4' : '-mt-4 flex-col',
          className,
        )}
        {...props}
      />
    </div>
  );
}

export function CarouselItem({ className, ...props }: ComponentProps<'div'>) {
  const { orientation } = useCarousel();
  return (
    <div
      data-slot="carousel-item"
      role="group"
      aria-roledescription="slide"
      className={cn(
        'min-w-0 shrink-0 grow-0 basis-full',
        orientation === 'horizontal' ? 'pl-4' : 'pt-4',
        className,
      )}
      {...props}
    />
  );
}
