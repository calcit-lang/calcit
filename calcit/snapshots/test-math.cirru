
{} (:package |test-math)
  :configs $ {} (:init-fn |test-math.main/main!) (:reload-fn |test-math.main/reload!)
  :files $ {}
    |test-math.main $ {}
      :ns $ quote
        ns test-math.main $ :require
      :defs $ {}
        |test-numbers $ quote
          defn test-numbers ()
            assert= 3 (+ 1 2)
            assert= 10 (+ 1 2 3 4)
            assert= 4 (- 10 1 2 3)
            assert= 24 (* 1 2 3 4)
            assert= 15 (/ 360 2 3 4)

            assert= (- 2) -2
            assert= (/ 2) 0.5

            assert-detect identity $ < 1 2 3 4 5
            assert-detect identity $ > 10 8 6 4
            assert-detect empty? nil
            assert-detect empty? ([])

            assert-detect identity $ <= 0 (rand) 100
            assert-detect identity $ <= 0 (rand 10) 10
            assert-detect identity $ <= 20 (rand 20 30) 30

            assert-detect identity $ <= 0 (rand-int) 100
            assert-detect identity $ <= 0 (rand-int 10) 10
            assert-detect identity $ <= 20 (rand-int 20 30) 30

            do true

        |log-title $ quote
          defn log-title (title)
            echo
            echo title
            echo

        |test-math $ quote
          defn test-math ()
            echo "|sin 1" $ sin 1
            echo "|cos 1" $ cos 1
            assert= 1 $ + (pow (sin 1) 2) (pow (cos 1) 2)
            assert= 1 $ floor 1.1
            assert= 2 $ ceil 1.1
            assert= 1 $ round 1.1
            assert= 2 $ round 1.8
            assert= 81 $ pow 3 4
            assert= 1 $ mod 33 4
            assert= 9 $ sqrt 81
            echo |PI &PI
            echo |E &E

        |test-compare $ quote
          defn test-compare ()
            assert= 4 $ max $ [] 1 2 3 4
            assert= 1 $ min $ [] 1 2 3 4

            assert-detect identity $ /= 1 2

        |test-hex $ quote
          fn ()
            log-title "|Testing hex"

            assert= 16 0x10
            assert= 15 0xf

        |main! $ quote
          defn main! ()
            log-title "|Testing numbers"
            test-numbers

            log-title "|Testing math"
            test-math

            log-title "|Testing compare"
            test-compare

            test-hex

            do true

      :proc $ quote ()
      :configs $ {} (:extension nil)
