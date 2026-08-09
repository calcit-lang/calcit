
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |test-string) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'test-string.main/main!) (:mode :native) (:reload-fn 'test-string.main/reload!)
      :modules $ [] |./util.cirru
      :type-slots $ {}
  :files $ {}
    |test-string.main $ %{} 'FileEntry
      :defs $ {}
        |main! $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing str") (test-str) (test-includes) (test-format) (test-char) (test-lisp-style) (test-bitwise) (do true)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-bitwise $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () $ inside-js:
              do
                assert= (bit-and 15 7) 7
                assert= (bit-and 16 7) 0
                assert= (bit-or 15 7) 15
                assert= (bit-or 16 7) 23
                assert= (bit-xor 15 7) 8
                assert= (bit-xor 16 7) 23
                assert= (bit-not 16) -17
                assert= (bit-not 0) -1
                assert= |0b10001 $ &number:display-by 17 2
                assert= |0o21 $ &number:display-by 17 8
                assert= |0x11 $ &number:display-by 17 16
          :examples $ []
          :schema $ :: 'Dynamic
        |test-char $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Test char")
              assert= 97 $ .get-char-code |a
              assert= 27721 $ .get-char-code "|汉"
              assert= |a $ char-from-code 97
              assert= (%some |a) (nth |abc 0)
              assert= (%some |b) (nth |abc 1)
              assert= (%some |a) (first |abc)
              assert= (%some |c) (last |abc)
              assert= (%none) (first |)
              assert= (%none) (last |)
          :examples $ []
          :schema $ :: 'Dynamic
        |test-format $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing format")
              assert= |1.2346 $ .format 1.23456789 4
              assert= |1.235 $ .format 1.23456789 3
              assert= |1.23 $ .format 1.23456789 2
              assert= |1.2 $ .format 1.23456789 1
              assert= |0x11 $ .display-by 17 16
              inside-eval: (; TODO not stable)
                ; assert= "|({} (:c ([] 3)) (:a 1) (:b |2) (:d ({} (([] 1 2) 3))))" $ to-lispy-string
                  {} (:a 1) (:b |2)
                    :c $ [] 3
                    :d $ {}
                        [] 1 2
                        , 3
                assert=
                  &cirru-quote:to-list $ cirru-quote
                    a b c $ d
                  [] |a |b |c $ [] |d
                assert= (.escape "|\n") "|\"\\n\""
                assert= (.escape "|\t") "|\"\\t\""
                assert= (.escape |a) "|\"a\""
              println |hashing: $ &hash 1
          :examples $ []
          :schema $ :: 'Dynamic
        |test-includes $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing includes")
              assert= true $ includes? |abc |abc
              assert= false $ includes? |abd |abc
              assert= (%some 3) (.find-index |0123456 |3)
              assert= (%some 3) (.find-index |0123456 |34)
              assert= (%some 0) (.find-index |0123456 |01)
              assert= (%some 4) (.find-index |0123456 |456)
              assert= (%none) (.find-index |0123456 |98)
              assert= true $ starts-with? |01234 |0
              assert= true $ starts-with? |01234 |01
              assert= false $ starts-with? |01234 |12
              assert= true $ starts-with? :a/b :a/
              assert= true $ starts-with? :a/b |a/
              assert= true $ ends-with? |01234 |34
              assert= true $ ends-with? |01234 |4
              assert= false $ ends-with? |01234 |23
              assert= |abc $ strip-prefix |ababc |ab
              assert= |0abc $ strip-prefix |0abc |ab
              assert= |aba $ strip-suffix |ababc |bc
              assert= |abc0 $ strip-suffix |abc0 |bc
          :examples $ []
          :schema $ :: 'Dynamic
        |test-lisp-style $ %{} 'CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Test lisp style")
              assert=
                format-to-lisp $ quote
                  defn f1 (x) (+ x y)
                , "|(defn f1 (x) (+ x y))"
              assert=
                format-to-lisp $ quote (nil? nil)
                , "|(nil? nil)"
              inside-eval: $ assert=
                format-to-cirru $ macroexpand-all
                  quote $ let
                      a 1
                      b :d
                      c |c
                    + a b c
                format-cirru $ []
                  [] |&let ([] |a |1)
                    [] |&let ([] |b |:d)
                      [] |&let ([] |c ||c) ([] |+ |a |b |c)
              assert=
                trim $ format-to-cirru
                  quote $ defn (a b) (+ a b)
                , "|defn (a b)\n  + a b"
              ; test format-cirru-one-liner
              assert=
                format-cirru-one-liner $ [] |defn
                  [] |add $ [] |a |b
                  [] |+ |a |b
                , "|defn (add (a b)) $ + a b"
              assert=
                format-cirru-one-liner $ [] |+ |1 |2
                , "|+ 1 2"
          :examples $ []
          :schema $ :: 'Dynamic
        |test-str $ %{} 'CodeEntry (:doc |)
          :code $ quote
            defn test-str ()
              assert=
                type-of $ &str 1
                , :string
              assert= (.replace "|this is a" |is |IS) "|thIS IS a"
              assert= |56789 $ .slice |0123456789 5
              assert= |567 $ .slice |0123456789 5 8
              assert= | $ .slice |0123456789 10
              assert= | $ .slice |0123456789 9 1
          :examples $ []
          :schema $ :: 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote
          ns test-string.main $ :require
            util.core :refer $ inside-eval: inside-js: log-title
