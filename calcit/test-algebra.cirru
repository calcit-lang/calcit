
{} (:package |test-algebra)
  :configs $ {} (:init-fn |test-algebra/main!) (:reload-fn |test-algebra/reload!)
    :modules $ [] |./util.cirru
  :files $ {}
    |test-algebra.main $ {}
      :ns $ quote
        ns test-algebra $ :require
          util.core :refer $ log-title inside-eval:
      :defs $ {}
        |main! $ quote
          defn main! ()
            log-title "|Testing algebra"
            ; "\"Experimental code, to simulate usages like Monad"
            test-map
            test-bind
            test-apply
            test-mappend

        |reload! $ quote
          defn reload! () nil

        |test-map $ quote
          defn test-map ()
            assert= nil $ .map nil inc
            assert=
              [] 11 12
              .map (' 1 2) $ fn (x) (+ x 10)
            assert=
              &{} :a 2 :b 3
              .map (&{} :a 1 :b 2) $ fn (pair)
                [] (first pair) (inc (last pair))
            let
                f1 $ fn (x) (+ x 10)
                f2 $ fn (x) (* x 2)
                f3 $ .map f1 f2
              assert= 16 (f3 3)

        |test-bind $ quote
          defn test-bind ()
            assert= nil $ .bind nil inc
            assert= ([] 0 1 0 1 2) $ .bind ([] 2 3) $ fn (x) (range x)
            let
                f1 $ fn (x) (+ x 10)
                f2 $ fn (x y) (* 2 x y)
                f3 $ .bind f1 f2
              assert= 78 (f3 3)

        |test-apply $ quote
          defn test-apply ()
            assert= nil
              .apply nil ([] inc)
            assert=
              [] 11 12 13 2 4 6
              .apply
                [] 1 2 3
                []
                  fn (x) (+ x 10)
                  fn (x) (* x 2)
            let
                f1 $ fn (x) (+ x 10)
                f2 $ fn (y z) (* 2 y z)
                f3 $ .apply f1 f2
              assert= 78 (f3 3)

        |test-mappend $ quote
          defn test-mappend ()
            assert= 1 $ .mappend nil 1
            assert= nil $ .mappend nil nil
            assert= |abcd $ .mappend |ab |cd
            assert= ([] 1 2 3 4) $ .mappend ([] 1 2) ([] 3 4)
            assert= (#{} 1 2 3 4) $ .mappend (#{} 1 2) (#{} 3 4)
            assert= (&{} :a 1 :b 2) $ .mappend (&{} :a 1) (&{} :b 2)

            let
                f1 $ fn (x) $ .slice x 1
                f2 $ fn (x) $ .slice x 0 $ dec $ count x
                f3 $ .mappend f1 f2
              assert= |234123
                f3 |1234
