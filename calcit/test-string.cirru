
{} (:package |test-string)
  :configs $ {} (:init-fn |test-string.main/main!) (:reload-fn |test-string.main/reload!)
  :files $ {}
    |test-string.main $ %{} :FileEntry
      :defs $ {}
        |log-title $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn log-title (title) (println) (println title) (println)
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (log-title "|Testing str") (test-str) (test-includes) (log-title "|Testing parse") (test-parse) (log-title "|Testing trim") (test-trim) (test-format) (test-char) (test-whitespace) (test-lisp-style) (test-methods) (test-bitwise) (do true)
        |test-bitwise $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn ()
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
        |test-char $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Test char")
              assert= 97 $ .get-char-code |a
              assert= 27721 $ .get-char-code "|汉"
              assert= |a $ char-from-code 97
              assert= |a $ nth |abc 0
              assert= |b $ nth |abc 1
              assert= |a $ first |abc
              assert= |c $ last |abc
              assert= nil $ first |
              assert= nil $ last |
        |test-format $ %{} :CodeEntry (:doc |)
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
        |test-includes $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Testing includes")
              assert= true $ includes? |abc |abc
              assert= false $ includes? |abd |abc
              assert= 3 $ .find-index |0123456 |3
              assert= 3 $ .find-index |0123456 |34
              assert= 0 $ .find-index |0123456 |01
              assert= 4 $ .find-index |0123456 |456
              assert= -1 $ .find-index |0123456 |98
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
        |test-lisp-style $ %{} :CodeEntry (:doc |)
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
                format-cirru-one-liner
                  [] |defn ([] |add ([] |a |b)) ([] |+ |a |b)
                , "|defn (add (a b)) $ + a b"
              assert=
                format-cirru-one-liner
                  [] |+ |1 |2
                , "|+ 1 2"
        |test-methods $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-methods () (log-title "|Testing string methods")
              assert= true $ .blank? |
              assert= true $ .blank? "| "
              assert= false $ .blank? |a
              assert= 3 $ .count |abc
              assert= | $ .empty |a
              assert= true $ .ends-with? |abc |c
              assert= false $ .ends-with? |abc |b
              assert= |a $ .get |abc 0
              assert= |b $ .get |abc 1
              assert= 1 $ .parse-float |1
              assert= 1.1 $ .parse-float |1.1
              assert= |Abcd $ .replace |abcd |a |A
              assert= |AbAd $ .replace |abad |a |A
              assert= ([] |a |c) (.split |abc |b)
              assert= ([] |a |c) (.split-lines "|a\nc")
              assert= true $ .starts-with? |abcd |a
              assert= false $ .starts-with? |abcd |b
              assert= |bcd $ .strip-prefix |abcd |a
              assert= |abc $ .strip-suffix |abcd |d
              assert= |abcd $ .strip-suffix |abcd |a
              assert= |bc $ .slice |abcd 1 3
              assert= |bcd $ .slice |abcd 1
              assert= "|文字" $ .slice "|中文字符串" 1 3
              assert= "|文字符串" $ .slice "|中文字符串" 1
              assert= "|ab cd" $ .trim "| ab cd"
              assert= true $ .empty? |
              assert= false $ .empty? |a
              assert= true $ .contains? |abcd 0
              assert= false $ .contains? |abcd 4
              assert= true $ .includes? |abcd |a
              assert= false $ .includes? |abcd |e
              assert= |a $ .nth |abc 0
              assert= |b $ .nth |abc 1
              assert= |a $ .first |abc
              assert= nil $ .first |
              assert= |bc $ .rest |abc
              assert= 0 $ .find-index |abc |a
              assert= 1 $ .find-index |abc |b
              assert= "|\"a \\\"\"" $ .escape "|a \""
              assert= |00000a $ .pad-left |a 6 |0
              assert= |a00000 $ .pad-right |a 6 |0
              assert= |12312a $ .pad-left |a 6 |123
              assert= |a12312 $ .pad-right |a 6 |123
        |test-parse $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () $ assert= 0 (parse-float |0)
        |test-str $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn test-str ()
              assert= (&str:concat |a |b) |ab
              assert= (&str:concat 1 2) |12
              assert= (str |a |b |c) |abc
              assert= (str |a nil |c) |ac
              assert= (str-spaced |a nil |c 12) "|a c 12"
              assert= (str-spaced nil nil |c 12) "|c 12"
              assert= (str-spaced |a nil |c 12 nil) "|a c 12"
              assert= (str 1 2 3) |123
              assert= "|(:: :a |世界 \"|海 洋\")"
                str $ :: :a |世界 "|海 洋"
              assert=
                type-of $ &str 1
                , :string
              assert= (.replace "|this is a" |is |IS) "|thIS IS a"
              assert= (split |a,b,c |,) ([] |a |b |c)
              assert= (split-lines "|a\nb\nc") ([] |a |b |c)
              assert= (split "|a中b文c" |) ([] |a "|中" |b "|文" |c)
              assert= 4 $ count |good
              assert= |56789 $ .slice |0123456789 5
              assert= |567 $ .slice |0123456789 5 8
              assert= | $ .slice |0123456789 10
              assert= | $ .slice |0123456789 9 1
              assert= -1 $ &str:compare |a |b
              assert= 1 $ &str:compare |b |a
              assert= 0 $ &str:compare |a |a
              assert-detect identity $ < |a |b
              assert-detect identity $ < |aa |ab
              assert-detect not $ > |aa |ab
        |test-trim $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn ()
              assert= | $ trim "|    "
              assert= |1 $ trim "|  1  "
              assert= |1 $ trim "|\n1\n"
              assert= | $ trim |______ |_
              assert= |1 $ trim |__1__ |_
        |test-whitespace $ %{} :CodeEntry (:doc |)
          :code $ quote
            fn () (log-title "|Test blank?")
              assert-detect identity $ blank? |
              assert-detect identity $ blank? "\""
              assert-detect identity $ blank? "| "
              assert-detect identity $ blank? "|  "
              assert-detect identity $ blank? "|\n"
              assert-detect identity $ blank? "|\n "
              assert-detect not $ blank? |1
              assert-detect not $ blank? "| 1"
              assert-detect not $ blank? "|1 "
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns test-string.main $ :require
            [] util.core :refer $ [] inside-eval:
