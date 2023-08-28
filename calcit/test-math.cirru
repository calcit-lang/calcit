
{} (:package |test-math)
  :configs $ {} (:init-fn |test-math.main/main!) (:reload-fn |test-math.main/reload!)
  :files $ {}
    |test-math.main $ %{} :FileEntry
      :defs $ {}
        |log-title $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn log-title (title) (println) (println title) (println)
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing numbers") (test-numbers) (log-title "|Testing math") (test-math) (log-title "|Testing compare") (test-compare) (test-hex) (test-integer) (test-methods) (test-bit-math) (do true)
        |test-bit-math $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|testing bit math")
              assert= 0 $ bit-shr 1 1
              assert= 1 $ bit-shr 2 1
              assert= 1 $ bit-shr 4 2
              assert= 2 $ bit-shl 1 1
              assert= 4 $ bit-shl 2 1
              assert= 16 $ bit-shl 4 2
        |test-compare $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-compare ()
              assert= 4 $ max ([] 1 2 3 4)
              assert= 1 $ min ([] 1 2 3 4)
              assert-detect identity $ /= 1 2
              assert= (&compare 1 |1) -1
              assert= (&compare |1 1) 1
              assert= (&compare 1 1) 0
              assert= (&compare |1 |1) 0
              assert= (&compare 1 :k) -1
              assert= (&compare :k |k) -1
              assert=
                &compare :k $ {}
                , -1
              assert=
                &compare :k $ []
                , -1
              assert=
                &compare :k $ #{}
                , -1
              assert=
                &compare :k $ :: 0 0
                , -1
        |test-hex $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing hex") (assert= 16 0x10) (assert= 15 0xf)
        |test-integer $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing integer")
              assert= true $ round? 1
              assert= false $ round? 1.1
        |test-math $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-math ()
              println "|sin 1" $ sin 1
              println "|cos 1" $ cos 1
              assert= 1 $ +
                pow (sin 1) 2
                pow (cos 1) 2
              assert= 1 $ floor 1.1
              assert= 2 $ ceil 1.1
              assert= 1 $ round 1.1
              assert= 2 $ round 1.8
              assert= 2 $ .round 1.8
              assert= 0.8 $ .fract 1.8
              assert= 81 $ pow 3 4
              assert= 1 $ &number:rem 33 4
              assert= 9 $ sqrt 81
              println |PI &PI
              println |E &E
        |test-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing number methods")
              assert= 1 $ .floor 1.1
              assert= 16 $ .pow 2 4
              assert= 2 $ .ceil 1.1
              assert= 0 $ .empty 1.1
              assert= 2.1 $ .inc 1.1
              assert= 1 $ .round 1.1
              assert= false $ .round? 1.1
              assert= true $ .round? 1
              assert= 2 $ .sqrt 4
              assert= 3 $ .rem 3 6
              assert= 2 $ .rem 11 3
              ; "has problem in comparing float numbers" $ assert= 0.1 (.fract 1.1)
        |test-numbers $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-numbers ()
              assert= 3 $ + 1 2
              assert= 10 $ + 1 2 3 4
              assert= 4 $ - 10 1 2 3
              assert= 24 $ * 1 2 3 4
              assert= 15 $ / 360 2 3 4
              assert= (- 2) -2
              assert= (/ 2) 0.5
              assert-detect identity $ < 1 2 3 4 5
              assert-detect identity $ > 10 8 6 4
              assert-detect empty? nil
              assert-detect empty? $ []
              do true
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns test-math.main $ :require
