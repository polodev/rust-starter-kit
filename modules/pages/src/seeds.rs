//! Starter content for the public pages, in English and Bangla.
//!
//! The seeder only inserts what is missing, so it is safe to run repeatedly and never overwrites
//! edits made later. The legal text is a starting point: have it reviewed before launch.

use loco_rs::Result;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use crate::entities::{page_translations, pages};

struct SeedTranslation {
    locale: &'static str,
    title: &'static str,
    meta_description: &'static str,
    body_html: &'static str,
}

struct SeedPage {
    slug: &'static str,
    sort_order: i32,
    translations: &'static [SeedTranslation],
}

const SEED_PAGES: &[SeedPage] = &[
    SeedPage {
        slug: "about",
        sort_order: 1,
        translations: &[
            SeedTranslation {
                locale: "en",
                title: "About us",
                meta_description: "Who we are, what we do and the values behind our work.",
                body_html: r##"<p>We build simple, dependable digital services. Our goal is to make technology easy to use and to give every visitor a clear, fast experience in the language they prefer.</p>
<h2>What we do</h2>
<p>We design, build and run web services for people and businesses. Everything on this website is available in English and Bangla, and you can switch language at any time from the top of the page.</p>
<h2>Our values</h2>
<ul>
<li><strong>Clarity.</strong> We explain things plainly and avoid hidden conditions.</li>
<li><strong>Reliability.</strong> We keep our services fast, secure and available.</li>
<li><strong>Respect.</strong> We treat your time and your data with care.</li>
</ul>
<h2>Contact us</h2>
<p>Questions or feedback? Write to us at <a href="mailto:contact@example.com">contact@example.com</a>.</p>"##,
            },
            SeedTranslation {
                locale: "bn",
                title: "আমাদের সম্পর্কে",
                meta_description: "আমরা কারা, কী করি এবং আমাদের কাজের মূল্যবোধ।",
                body_html: r##"<p>আমরা সহজ ও নির্ভরযোগ্য ডিজিটাল সেবা তৈরি করি। প্রযুক্তিকে ব্যবহারে সহজ করা এবং প্রতিটি দর্শককে তাঁর পছন্দের ভাষায় পরিষ্কার ও দ্রুত অভিজ্ঞতা দেওয়াই আমাদের লক্ষ্য।</p>
<h2>আমরা কী করি</h2>
<p>আমরা ব্যক্তি ও প্রতিষ্ঠানের জন্য ওয়েব সেবা নকশা, নির্মাণ ও পরিচালনা করি। এই ওয়েবসাইটের সবকিছু ইংরেজি ও বাংলা দুই ভাষাতেই পাওয়া যায়, এবং পৃষ্ঠার উপরের অংশ থেকে যেকোনো সময় ভাষা বদলানো যায়।</p>
<h2>আমাদের মূল্যবোধ</h2>
<ul>
<li><strong>স্বচ্ছতা।</strong> আমরা বিষয়গুলো সহজ ভাষায় ব্যাখ্যা করি এবং কোনো গোপন শর্ত রাখি না।</li>
<li><strong>নির্ভরযোগ্যতা।</strong> আমাদের সেবা দ্রুত, নিরাপদ ও সবসময় চালু রাখার চেষ্টা করি।</li>
<li><strong>শ্রদ্ধা।</strong> আপনার সময় ও তথ্যকে আমরা যত্নের সঙ্গে দেখি।</li>
</ul>
<h2>যোগাযোগ</h2>
<p>কোনো প্রশ্ন বা মতামত থাকলে লিখুন <a href="mailto:contact@example.com">contact@example.com</a> ঠিকানায়।</p>"##,
            },
        ],
    },
    SeedPage {
        slug: "privacy-policy",
        sort_order: 2,
        translations: &[
            SeedTranslation {
                locale: "en",
                title: "Privacy Policy",
                meta_description: "How we collect, use and protect your information.",
                body_html: r##"<p>This policy explains what information we collect when you use this website, how we use it and the choices you have.</p>
<h2>1. Information we collect</h2>
<ul>
<li><strong>Information you give us,</strong> such as your name, email address or messages you send us.</li>
<li><strong>Technical information,</strong> such as your browser type, device and the pages you visit, collected through server logs.</li>
</ul>
<h2>2. How we use your information</h2>
<p>We use your information to provide and improve our services, to respond to you, to keep the website secure and to meet legal obligations. We do not sell your personal information.</p>
<h2>3. Cookies</h2>
<p>We use a small number of cookies. A <em>language</em> cookie remembers whether you prefer English or Bangla, and a <em>session</em> cookie keeps you signed in when you have an account. You can delete or block cookies in your browser settings, but parts of the website may then not work as expected.</p>
<h2>4. Sharing</h2>
<p>We share information only with service providers who help us run the website, when the law requires it, or with your consent.</p>
<h2>5. Retention and security</h2>
<p>We keep information only as long as needed for the purposes above and protect it with reasonable technical and organisational measures.</p>
<h2>6. Your choices</h2>
<p>You may ask to see, correct or delete the personal information we hold about you by contacting us.</p>
<h2>7. Changes to this policy</h2>
<p>We may update this policy from time to time. The latest version is always on this page.</p>
<h2>8. Contact</h2>
<p>Privacy questions can be sent to <a href="mailto:privacy@example.com">privacy@example.com</a>.</p>"##,
            },
            SeedTranslation {
                locale: "bn",
                title: "গোপনীয়তা নীতি",
                meta_description: "আমরা কীভাবে আপনার তথ্য সংগ্রহ, ব্যবহার ও সুরক্ষা করি।",
                body_html: r##"<p>এই নীতিতে ব্যাখ্যা করা হয়েছে, আপনি এই ওয়েবসাইট ব্যবহার করলে আমরা কোন তথ্য সংগ্রহ করি, কীভাবে তা ব্যবহার করি এবং আপনার কী কী সিদ্ধান্ত নেওয়ার সুযোগ আছে।</p>
<h2>১. আমরা যে তথ্য সংগ্রহ করি</h2>
<ul>
<li><strong>আপনার দেওয়া তথ্য,</strong> যেমন আপনার নাম, ইমেইল ঠিকানা বা আমাদের পাঠানো বার্তা।</li>
<li><strong>প্রযুক্তিগত তথ্য,</strong> যেমন আপনার ব্রাউজারের ধরন, ডিভাইস এবং কোন কোন পৃষ্ঠা দেখেছেন, যা সার্ভার লগের মাধ্যমে সংগৃহীত হয়।</li>
</ul>
<h2>২. তথ্য ব্যবহারের উদ্দেশ্য</h2>
<p>আমরা আপনার তথ্য ব্যবহার করি সেবা দিতে ও উন্নত করতে, আপনার প্রশ্নের উত্তর দিতে, ওয়েবসাইট নিরাপদ রাখতে এবং আইনি বাধ্যবাধকতা পালন করতে। আমরা আপনার ব্যক্তিগত তথ্য বিক্রি করি না।</p>
<h2>৩. কুকিজ</h2>
<p>আমরা অল্প কিছু কুকি ব্যবহার করি। <em>ভাষা</em> কুকি মনে রাখে আপনি ইংরেজি না বাংলা পছন্দ করেন, আর <em>সেশন</em> কুকি অ্যাকাউন্ট থাকলে আপনাকে লগইন অবস্থায় রাখে। ব্রাউজারের সেটিংস থেকে কুকি মুছে ফেলা বা বন্ধ করা যায়, তবে তখন ওয়েবসাইটের কিছু অংশ ঠিকভাবে কাজ নাও করতে পারে।</p>
<h2>৪. তথ্য ভাগাভাগি</h2>
<p>আমরা শুধু ওয়েবসাইট পরিচালনায় সহায়তাকারী সেবাদাতাদের সঙ্গে, আইনের প্রয়োজনে অথবা আপনার সম্মতিতে তথ্য ভাগ করি।</p>
<h2>৫. সংরক্ষণ ও নিরাপত্তা</h2>
<p>উপরের উদ্দেশ্যে যতদিন প্রয়োজন, আমরা ততদিনই তথ্য সংরক্ষণ করি এবং যুক্তিসংগত প্রযুক্তিগত ও প্রাতিষ্ঠানিক ব্যবস্থায় তা সুরক্ষিত রাখি।</p>
<h2>৬. আপনার সিদ্ধান্ত</h2>
<p>আপনার সম্পর্কে আমাদের কাছে থাকা ব্যক্তিগত তথ্য দেখতে, সংশোধন করতে বা মুছে ফেলতে চাইলে আমাদের সঙ্গে যোগাযোগ করুন।</p>
<h2>৭. নীতিতে পরিবর্তন</h2>
<p>আমরা সময়ে সময়ে এই নীতি হালনাগাদ করতে পারি। সর্বশেষ সংস্করণ সবসময় এই পৃষ্ঠায় থাকবে।</p>
<h2>৮. যোগাযোগ</h2>
<p>গোপনীয়তা বিষয়ক প্রশ্ন পাঠান <a href="mailto:privacy@example.com">privacy@example.com</a> ঠিকানায়।</p>"##,
            },
        ],
    },
    SeedPage {
        slug: "terms-of-service",
        sort_order: 3,
        translations: &[
            SeedTranslation {
                locale: "en",
                title: "Terms of Service",
                meta_description:
                    "The terms that apply when you use this website and its services.",
                body_html: r##"<p>By using this website you agree to these terms. If you do not agree, please do not use it.</p>
<h2>1. Using the website</h2>
<p>You may use the website only for lawful purposes. You must not try to disrupt it, gain unauthorised access, or use it to harm others.</p>
<h2>2. Accounts</h2>
<p>If you create an account, keep your password secret and give us accurate information. You are responsible for activity under your account.</p>
<h2>3. Content and intellectual property</h2>
<p>The website, its design and its content belong to us or our licensors and are protected by law. You may not copy or redistribute them without permission, except as the law allows.</p>
<h2>4. Disclaimer</h2>
<p>The website and its services are provided &ldquo;as is&rdquo;. We work to keep them accurate and available, but we cannot promise they will always be error-free or uninterrupted.</p>
<h2>5. Limitation of liability</h2>
<p>To the extent the law allows, we are not liable for indirect or consequential loss arising from your use of the website.</p>
<h2>6. Suspension and termination</h2>
<p>We may suspend or end access to the website or an account if these terms are broken.</p>
<h2>7. Changes to these terms</h2>
<p>We may update these terms from time to time. Continued use of the website after a change means you accept the updated terms.</p>
<h2>8. Contact</h2>
<p>Questions about these terms can be sent to <a href="mailto:contact@example.com">contact@example.com</a>.</p>"##,
            },
            SeedTranslation {
                locale: "bn",
                title: "পরিষেবার শর্তাবলি",
                meta_description: "এই ওয়েবসাইট ও এর সেবা ব্যবহারের ক্ষেত্রে প্রযোজ্য শর্তাবলি।",
                body_html: r##"<p>এই ওয়েবসাইট ব্যবহার করলে আপনি এই শর্তাবলিতে সম্মত হচ্ছেন। সম্মত না হলে অনুগ্রহ করে এটি ব্যবহার করবেন না।</p>
<h2>১. ওয়েবসাইট ব্যবহার</h2>
<p>আপনি শুধু বৈধ উদ্দেশ্যে ওয়েবসাইট ব্যবহার করতে পারবেন। এটি বিঘ্নিত করার চেষ্টা, অনুমতি ছাড়া প্রবেশ বা অন্যের ক্ষতি করতে এটি ব্যবহার করা যাবে না।</p>
<h2>২. অ্যাকাউন্ট</h2>
<p>অ্যাকাউন্ট খুললে পাসওয়ার্ড গোপন রাখুন এবং সঠিক তথ্য দিন। আপনার অ্যাকাউন্টের সব কার্যক্রমের দায় আপনার।</p>
<h2>৩. কনটেন্ট ও মেধাস্বত্ব</h2>
<p>ওয়েবসাইট, এর নকশা ও কনটেন্ট আমাদের বা আমাদের লাইসেন্সদাতাদের এবং আইন দ্বারা সুরক্ষিত। আইনে অনুমোদিত ক্ষেত্র ছাড়া অনুমতি ছাড়া এগুলো কপি বা পুনর্বিতরণ করা যাবে না।</p>
<h2>৪. দায় অস্বীকার</h2>
<p>ওয়েবসাইট ও এর সেবা &ldquo;যেমন আছে&rdquo; অবস্থায় দেওয়া হয়। আমরা এগুলো নির্ভুল ও চালু রাখার চেষ্টা করি, তবে সবসময় ত্রুটিমুক্ত বা নিরবচ্ছিন্ন থাকবে, এমন প্রতিশ্রুতি দিতে পারি না।</p>
<h2>৫. দায়ের সীমাবদ্ধতা</h2>
<p>আইন যতটুকু অনুমোদন করে, ওয়েবসাইট ব্যবহারের ফলে সৃষ্ট পরোক্ষ বা পরবর্তী ক্ষতির জন্য আমরা দায়ী নই।</p>
<h2>৬. স্থগিতকরণ ও বাতিল</h2>
<p>এই শর্তাবলি ভঙ্গ হলে আমরা ওয়েবসাইট বা কোনো অ্যাকাউন্টে প্রবেশাধিকার স্থগিত বা বাতিল করতে পারি।</p>
<h2>৭. শর্তাবলিতে পরিবর্তন</h2>
<p>আমরা সময়ে সময়ে এই শর্তাবলি হালনাগাদ করতে পারি। পরিবর্তনের পরও ওয়েবসাইট ব্যবহার চালিয়ে গেলে আপনি হালনাগাদ শর্তাবলি মেনে নিচ্ছেন বলে গণ্য হবে।</p>
<h2>৮. যোগাযোগ</h2>
<p>এই শর্তাবলি নিয়ে প্রশ্ন পাঠান <a href="mailto:contact@example.com">contact@example.com</a> ঠিকানায়।</p>"##,
            },
        ],
    },
];

/// Inserts every missing page and translation. Existing rows are left untouched.
pub async fn run(db: &DatabaseConnection) -> Result<()> {
    for seed in SEED_PAGES {
        let page = match pages::Entity::find()
            .filter(pages::Column::Slug.eq(seed.slug))
            .one(db)
            .await?
        {
            Some(page) => page,
            None => {
                pages::ActiveModel {
                    slug: Set(seed.slug.to_string()),
                    is_published: Set(true),
                    sort_order: Set(seed.sort_order),
                    ..Default::default()
                }
                .insert(db)
                .await?
            }
        };

        for translation in seed.translations {
            let exists = page_translations::Entity::find()
                .filter(page_translations::Column::PageId.eq(page.id))
                .filter(page_translations::Column::Locale.eq(translation.locale))
                .one(db)
                .await?
                .is_some();
            if exists {
                continue;
            }
            page_translations::ActiveModel {
                page_id: Set(page.id),
                locale: Set(translation.locale.to_string()),
                title: Set(translation.title.to_string()),
                meta_description: Set(translation.meta_description.to_string()),
                body_html: Set(translation.body_html.to_string()),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
    }
    Ok(())
}
