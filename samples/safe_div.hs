module SafeDiv (safeDiv, divisors) where

-- | integer division that returns Nothing on division by zero.
safeDiv :: Integral a => a -> a -> Maybe a
safeDiv _ 0 = Nothing
safeDiv x y = Just (x `div` y)

-- | all divisors of n in ascending order.
divisors :: Int -> [Int]
divisors n = [d | d <- [1 .. n], n `mod` d == 0]
